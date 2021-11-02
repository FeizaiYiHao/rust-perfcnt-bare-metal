//! This is the controler for IA32_PERF_GLOBAL_STATUS, IA32_PERF_GLOBAL_OVF_CTRL and  IA32_PERF_GLOBAL_CTRL MSRs.
//! 
//! OS should obtain one instance of the controler before constrcuting any PerfCounter.
//! 
//! Must call init() before use.
//! 

use x86::{msr::{rdmsr, wrmsr}, perfcnt::intel::Counter};
use crate::ErrorMsg;

pub struct PerfCounterControler{
    version_identifier:u8,
    number_msr:u8,
    bit_width:u8,
    events_available:u8,
    number_fixed_function_counter:u8,
    bit_width_fixed_counter:u8,
    unavailable_events_vec:u8,
    perf_capability: bool,
}

impl  PerfCounterControler{
    pub fn new() -> PerfCounterControler{
        PerfCounterControler{
            version_identifier:0,
            number_msr:0,
            bit_width:0,
            events_available:0,
            number_fixed_function_counter:0,
            bit_width_fixed_counter:0,
            unavailable_events_vec:0,
            perf_capability:false,
        }
    }

    ///This must be called. 
    pub fn init(&mut self){
        let mut rax :u64;
        let mut rdx :u64;
        let mut rbx :u64;
        unsafe{
        //get CPUID:0AH info
            asm!(
                "MOV EAX, 0AH",
                "CPUID",
                "MOV R8, RBX",
                out("rax") rax,
                out("rdx") rdx,
               out("r8") rbx,
            );
        } 
        let mask:u64 =  255;
        self.version_identifier = (rax & mask) as u8;
        self.number_msr = ((rax >> 8) & mask) as u8;
        self.bit_width =  if rax & mask != 0 {((rax >> 16) & mask) as u8} else {40 as u8};
        self.events_available = ((rax >> 24) & mask )as u8;
        self.number_fixed_function_counter = (rdx & 31 )as u8;
        self.bit_width_fixed_counter = (rdx>>5 & 127) as u8;
        self.unavailable_events_vec = (rbx & mask) as u8;
        unsafe{
            let mut rcx :u64;
            asm!(
                "MOV EAX, 01H",
                "CPUID",
                "MOV R8, RBX",
               out("rcx") rcx,
            );
            if (rcx >> 15)& 1 == 1{
                self.perf_capability = (rdmsr(x86::msr::IA32_PERF_CAPABILITIES)>>13 & 0x1) == 1;
            }
            else{
                self.perf_capability = false;
            }

            if ! self.perf_capability{
                self.bit_width = 32;
            }

            }
    }

    pub fn get_version_identifier(&self)-> u8{
        self.version_identifier
    }
    pub fn get_number_msr(&self)-> u8{
        self.number_msr
    }
    pub fn get_number_fixed_function_counter(&self)-> u8{
        self.number_fixed_function_counter
    }
    pub fn get_bit_width(&self)-> u8{
        self.bit_width
    }
    pub fn get_events_available(&self)-> u8{
        self.events_available
    }
    pub fn get_bit_width_fixed_counter(&self)-> u8{
        self.bit_width_fixed_counter
    }
    pub fn get_unavailable_events_vec(&self)-> u8{
        self.unavailable_events_vec
    }

    pub fn get_perf_capability(&self)->bool{
        self.perf_capability
    }
    
    ///Will clear overflow indicator for corresponding pmc in IA32_PERF_GLOBAL_STATUS
    pub fn clear_overflow_bit(&self, c:Counter){
        match c {
            Counter::Fixed(index) => {
                let v = self.read_overflow_ctrl();
                let v_tmp = v | (1<<(index+32));
                self.set_overflow_ctrl(v_tmp);  
                self.set_overflow_ctrl(v);  
            },
            Counter::Programmable(index) => {
                let v = self.read_overflow_ctrl();
                let v_tmp = v | (1<<index);
                self.set_overflow_ctrl(v_tmp);  
                self.set_overflow_ctrl(v);  
            },
        }
    }

    ///After one overflow PMI occurrence, the following PMI will be masked.
    /// This function clears the mask and enables future PMIs.
    /// Should probably be called in interrput handler.
    pub fn reset_overflow_interrput(&self){
        let mask:u32 = !(1<<16);
        let mut eax:u32;
        let rdx:u64 = 0xFEE00340;
        unsafe{
            asm!("MOV eax, [edx]",
            in("rdx") rdx,
            out("eax") eax,
            );
        }
        eax = eax & mask;
        unsafe{
            asm!("MOV [edx],eax",
            in("rdx") rdx,
            in("eax") eax,
            );
        }
    }

    ///Start generating PMI on pmc overflow.
    /// Use get_overflow_counter() to find out which counter overflows.
    pub fn register_overflow_interrput(&self, interrput_vec:u8){
        unsafe{
            let edx:u32 = 0xFEE00340; //APIC PMC register 
            let eax:u64 = interrput_vec as u64;
            asm!("MOV [edx],eax",
            in("edx") edx,
            in("eax") eax,
            );
        }
    }

    pub fn read_globle_ctrl_bits(&self)->Result<u64,ErrorMsg>{
        if self.get_version_identifier()>=2{
            unsafe{Ok(rdmsr(0x38f))}
        }
        else{
            Err(ErrorMsg::UnsupportedVersion)
        }
        
    }

    pub fn set_globle_ctrl(&self,value:u64){
        if self.get_version_identifier()>=2{
            let rcx:u64 = 0x38f;
            let rax:u32 = value as u32;
            let rdx:u32 = (value>>32) as u32;
            unsafe {
                asm!(
                    //MSR[ECX] := EDX:EAX;
                "wrmsr",
                in("rcx") rcx,
                in("rax") rax,
                in("rdx") rdx,
            )
            }
        }
    }

    ///Set enable bit for the counter in IA32_PERF_GLOBAL_CTL.
    /// Also need to set enable bit in the specific pmc_ctl MSR to enable the counter
    pub fn enable_counter(&self,c:Counter){
        if self.get_version_identifier() >= 2{
            match c {
                Counter::Fixed(index) => {
                    let bits = self.read_globle_ctrl_bits().unwrap();
                    self.set_globle_ctrl(bits | (1<<(index + 32)));
                },
                Counter::Programmable(index) => {
                    let bits = self.read_globle_ctrl_bits().unwrap();
                    self.set_globle_ctrl(bits | (1<<(index )));
                },
            }
        }
    }

    ///Clear enable bit for the counter in IA32_PERF_GLOBAL_CTL.
    /// counter is disable if one of the following is clear:
    ///  enable bit in  pmc_ctl MSR or
    ///  enable bit in IA32_PERF_GLOBAL_CTL
    pub fn disable_counter(&self,c:Counter){
        if self.get_version_identifier() >= 2{
            match c {
                Counter::Fixed(index) => {
                    let bits = self.read_globle_ctrl_bits().unwrap();
                    self.set_globle_ctrl(bits & (!(1<<(index + 32))));
                },
                Counter::Programmable(index) => {
                    let bits = self.read_globle_ctrl_bits().unwrap();
                    self.set_globle_ctrl(bits & ( ! (1<<(index ))));
                },
            }
        }
    }

    pub fn read_overflow_status(&self)->u64{
        unsafe{
            rdmsr(0x38E)
        }
    }

    pub fn set_overflow_status(&self, value:u64){
        unsafe{
            wrmsr(0x38E,value)
        }
    }

    pub fn read_overflow_ctrl(&self)->u64{
        unsafe{
            rdmsr(0x390)
        }
    }

    pub fn set_overflow_ctrl(&self, value:u64){
        unsafe{
            wrmsr(0x390,value)
        }
    }

    ///Get the overflowe counter during a PMI.
    /// Should only overflow one counter at a time.
    pub fn get_overflow_counter(&self) -> Option<Counter>{
            let reading = self.read_overflow_status();
            for i in 0..63{
                if ((reading >> i) & 1 ) != 0{
                    if i < 32{
                        return Some(Counter::Programmable(i));
                    }
                    else{
                        return Some(Counter::Fixed(i-32));
                    }
                }
            }
            return None;
        
    }

    pub fn check_if_general_pmc_is_in_use(&self,index:u8)->bool{
        let mut ret:bool = true;
        unsafe{
            let mask = rdmsr(0x186+index as u32);
            ret = ret & (mask>>22 & 1 > 0);

            if self.get_version_identifier()>=2{
                let mask = rdmsr(0x38f);
                ret = ret & (mask>>index & 1 > 0);
            }
        }
        ret
    }


    pub fn check_if_fixed_pmc_is_in_use(&self,index:u8)->bool{
        let mut ret:bool;
        unsafe{
            let mask = rdmsr(0x38D+index as u32);
            ret = (mask>>(4*index)&3) > 0;

            let mask = rdmsr(0x38f);
            ret = ret & (mask>>(index + 32) > 0);
        }
        ret
    }

    ///Check if the counter is in use
    /// Should call before start()
    pub fn check_in_use(&self,c:Counter)->bool{
        match c{
            Counter::Fixed(index) => self.check_if_fixed_pmc_is_in_use(index),
            Counter::Programmable(index) => self.check_if_general_pmc_is_in_use(index),
        }
    }
}

pub static mut PERFCNT_GLOBAL_CTRLER:PerfCounterControler = PerfCounterControler{
    version_identifier:0,
    number_msr:0,
    bit_width:0,
    events_available:0,
    number_fixed_function_counter:0,
    bit_width_fixed_counter:0,
    unavailable_events_vec:0,
    perf_capability:false,
};