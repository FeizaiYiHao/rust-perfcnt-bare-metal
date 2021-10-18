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
        }
    }

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
        self.bit_width =  if rax & mask == 0 {((rax >> 16) & mask) as u8} else {40 as u8};
        self.events_available = ((rax >> 24) & mask )as u8;
        self.number_fixed_function_counter = (rdx & 31 )as u8;
        self.bit_width_fixed_counter = (rdx>>5 & 127) as u8;
        self.unavailable_events_vec = (rbx & mask) as u8;
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

    pub fn reset_overflow_interrput(&self){
        let mask:u32 = !(1<<16);
        unsafe{
            let edx:u32 = 0xFEE00340;
            let eax:u32 = 0x000000E2;
            asm!("MOV [edx],eax",
            in("edx") edx,
            in("eax") eax,
            );
        }
    }

    pub fn set_overflow_interrput(&self){
        let mask:u32 = !(1<<16);
        unsafe{
            let edx:u32 = 0xFEE00340;
            let eax:u32 = 0x000000E2;
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

    pub fn enable_counter(&self,c:Counter){
        if self.get_version_identifier() >= 2{
            match c {
                Counter::Fixed(index) => {
                    let bits = self.read_globle_ctrl_bits().unwrap();
                    self.set_globle_ctrl(bits & (1<<(index + 32)));
                },
                Counter::Programmable(index) => {
                    let bits = self.read_globle_ctrl_bits().unwrap();
                    self.set_globle_ctrl(bits & (1<<(index )));
                },
            }
        }
    }

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
}

pub static mut PERFCNT_GLOBAL_CTRLER:PerfCounterControler = PerfCounterControler{
    version_identifier:0,
    number_msr:0,
    bit_width:0,
    events_available:0,
    number_fixed_function_counter:0,
    bit_width_fixed_counter:0,
    unavailable_events_vec:0,
};