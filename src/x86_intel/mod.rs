use crate::AbstractPerfCounter;
use x86::{msr::*, perfcnt::intel::{ EventDescription,Counter,Tuple}};


pub enum ErrorMsg {
    CounterInUse,
    UnsupportedEvent,
    CounterOutOfRange,
    UnsupportedFixPMC,
}

pub const ENABLE_GENERAL_PMC_MASK: u64 = 0x1<<22;


pub struct PerfCounter{
    version_identifier:u8,
    number_msr:u8,
    bit_width:u8,
    events_available:u8,
    number_fixed_function_counter:u8,
    bit_width_fixed_counter:u8,
    unavailable_events_vec:u8,
    pmc_index:u8,    
    counter_type:Counter,
    general_pmc_mask:u64,
    fixed_pmc_ring_lv:u8, //0 for disable, 1 for OS, 2 for USER, 3 for ALL
    is_fixed_pmc_pmi_enabled:bool,
}



impl  PerfCounter{
    pub fn new()->PerfCounter{
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
        PerfCounter{
            version_identifier : (rax & mask) as u8,
            number_msr : ((rax >> 8) & mask) as u8,
            bit_width :  if rax & mask == 0 {((rax >> 16) & mask) as u8} else {40 as u8}, 
            events_available : ((rax >> 24) & mask )as u8,
            number_fixed_function_counter : (rdx & 31 )as u8,
            bit_width_fixed_counter : (rdx>>5 & 127) as u8,
            unavailable_events_vec : (rbx & mask) as u8,
            pmc_index: 0,
            counter_type: Counter::Programmable(0),
            general_pmc_mask: 0,
            fixed_pmc_ring_lv: 0,
            is_fixed_pmc_pmi_enabled: false,
        }
    }

    pub fn build_from_intel_hw_event(&mut self,event:&EventDescription,index:u8,is_user_enabled:bool,is_os_enabled:bool)->Result<(),ErrorMsg>{

        match event.counter{

            Counter::Fixed(index)=> 
            if self.version_identifier<2 {
                return Err(ErrorMsg::UnsupportedFixPMC);
            }else if index > self.number_fixed_function_counter{
                    return Err(ErrorMsg::CounterOutOfRange);
            }


            Counter::Programmable(_) => 
            if index > self.number_msr{
                return Err(ErrorMsg::CounterOutOfRange);
            }else{
                self.pmc_index = index;
                self.counter_type = event.counter;
                let mut config: u64 = 0;

                match event.event_code {
                Tuple::One(code) => config |= (code as u64) << 0,
                Tuple::Two(_, _) => unreachable!(), // NYI
                };
                match event.umask {
                Tuple::One(code) => config |= (code as u64) << 8,
                Tuple::Two(_, _) => unreachable!(), // NYI
            };
            config |= (event.counter_mask as u64) << 24;

            if event.edge_detect {
                config |= 1 << 18;
            }
            if event.any_thread {
                config |= 1 << 21;
            }
            if event.invert {
                config |= 1 << 23;
            }
            if is_user_enabled{
                config |= 1 <<16;
            }
            if is_os_enabled{
                config |= 1 <<17;
            }
            self.general_pmc_mask = config | ENABLE_GENERAL_PMC_MASK;
            
            }

            
        }

        Ok(())
    }

    pub fn build_general_from_raw(&mut self,eventmask:u32,umask:u32,user_enabled:bool,os_enabled:bool,counter_mask:u8,edge_detect:bool,pmc_index:u8){
        self.general_pmc_mask = 0;
        self.general_pmc_mask |= (eventmask & 0xFF)as u64;
        self.general_pmc_mask |= ((umask<<8) & (0xFF<<8)) as u64;
        self.general_pmc_mask |= (((user_enabled as u64) <<16) & (0x1<<16)) as u64;
        self.general_pmc_mask |= (((os_enabled as u64)<<17) & (0x1<<17)) as u64;
        self.general_pmc_mask |= ENABLE_GENERAL_PMC_MASK;
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
    pub fn get_pmc_index(&self)-> u8{
        self.pmc_index
    }
    pub fn get_counter_type(&self)-> Counter{
        self.counter_type
    }
    pub fn get_general_pmc_mask(&self)-> u64{
        self.general_pmc_mask
    }
    pub fn get_fixed_pmc_ring_lv(&self)->u8{
        self.fixed_pmc_ring_lv
    }
    pub fn get_is_fixed_pmc_pmi_enabled(&self)->bool{
        self.is_fixed_pmc_pmi_enabled
    }

    pub fn read_general_pmc_ctr(&self, index:u8)->u64{
        let  rcx:u64 = (0+index) as u64;
        let mut rax:u64;
        let mut rdx:u64;
        unsafe{
            //get general_pmc reading at index
            asm!(
                "rdpmc",
                in("rcx") rcx,
                out("rax") rax,
                out("rdx") rdx,
            );
        } 
        (rax<<32>>32) | rdx<<32
    }

    pub fn read_fixed_pmc_ctr(&self, index:u8)->u64{
        let  rcx:u64 = (0+index) as u64 | (1<<30);
        let mut rax:u64;
        let mut rdx:u64;
        unsafe{
            //get fix_pmc reading at index
            asm!(
                "rdpmc",
                in("rcx") rcx,
                out("rax") rax,
                out("rdx") rdx,
            );
        } 
        (rax<<32>>32) | rdx<<32
    }

    pub fn set_general_pmc_ctr(&self, index:u8,value:u64){
        unsafe {wrmsr(IA32_A_PMC0+index as u32, value)}
    }

    pub fn set_general_pmc_ctrl(&self, mask:u64,index:u8){
        unsafe {wrmsr(0x186+index as u32, mask)}
    }

    pub fn set_fixed_pmc_ctr(&self, mask:u64,index:u8){
        unsafe {wrmsr(0x309+index as u32, mask)}
    }

    pub fn set_fixed_pmc_ctrl(&self, index:u8,enabled_ring_lv:u8,is_pmi_enabled:bool){
        let rcx:u32 = 0x38D ;
        let rax:u64 = ((if is_pmi_enabled {8+enabled_ring_lv} else {enabled_ring_lv}) << (index * 4)) as u64; 
       unsafe{ wrmsr(rcx, rax)}
    }

    pub fn enable_general_pmc(&self,index:u8){

        if self.get_version_identifier()>=2{
            let rcx:u64 = 0x38f;
            unsafe{
                let msr:u64 = rdmsr(0x38f as u32);
                let rax:u32 = 1<<index | (msr as u32);
                let rdx:u32 = 0 | ((msr>>32) as u32);
                asm!(
                    //MSR[ECX] := EDX:EAX;
                    "wrmsr",
                    in("rcx") rcx,
                    in("rax") rax,
                    in("rdx") rdx,
                )
            }
        }

        unsafe {wrmsr(0x186+index as u32, self.get_general_pmc_mask());}
    }

    pub fn disable_general_pmc(&self,index:u8){

        if self.get_version_identifier()>=2{
            let rcx:u64 = 0x38f;
            unsafe{
                let msr:u64 = rdmsr(0x38f as u32);
                let rax:u32 = 1<<index^0 & (msr as u32);
                let rdx:u32 = 0<<index^0 & ((msr>>32) as u32);

                asm!(
                    //MSR[ECX] := EDX:EAX;
                    "wrmsr",
                    in("rcx") rcx,
                    in("rax") rax,
                    in("rdx") rdx,
                )
            }
        }

        unsafe {wrmsr(0x186+index as u32, 0);}
    }

    pub fn enable_fixed_pmc(&self,index:u8){
        let rcx:u64 = 0x38f;
        unsafe{
            let msr:u64 = rdmsr(0x38f as u32);
            let rax:u32 = 1<<(31+index) | (msr as u32);
            let rdx:u32 = 0 | ((msr>>32) as u32);
            asm!(
                //MSR[ECX] := EDX:EAX;
                "wrmsr",
                in("rcx") rcx,
                in("rax") rax,
                in("rdx") rdx,
            )
        }

        let rcx:u32 = 0x38D ;
        let rax:u64 = ((0x15<< (index * 4)) as u64) ^ 0; 
       unsafe{ wrmsr(rcx, rdmsr(rcx)&rax);}
    }

    pub fn disable_fixed_pmc(&self,index:u8){
        let rcx:u64 = 0x38f;
        unsafe{
            let msr:u64 = rdmsr(0x38f as u32);
            let rax:u32 = 1<<(31+index)^0 & (msr as u32);
            let rdx:u32 = 0^0 & ((msr>>32) as u32);
            asm!(
                //MSR[ECX] := EDX:EAX;
                "wrmsr",
                in("rcx") rcx,
                in("rax") rax,
                in("rdx") rdx,
            )
        }

        let rcx:u32 = 0x38D ;
        let rax:u64 = ((if self.get_is_fixed_pmc_pmi_enabled() {8+self.get_fixed_pmc_ring_lv()} else {self.get_fixed_pmc_ring_lv()}) << (index * 4)) as u64; 
       unsafe{ wrmsr(rcx, rdmsr(rcx)|rax);}
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

impl<'a> AbstractPerfCounter for PerfCounter {
    fn reset(&self) -> Result<(),ErrorMsg> {
        match self.get_counter_type(){
            Counter::Programmable(_) => self.set_general_pmc_ctr(self.get_pmc_index(),0),
            Counter::Fixed(_) => self.set_fixed_pmc_ctr(0, self.get_pmc_index()),
        };
        Ok(())
    }

    fn start(&self) -> Result<(), ErrorMsg> {
        match self.get_counter_type(){
            Counter::Programmable(_)=>{
                self.enable_general_pmc(self.get_pmc_index())
            }
            Counter::Fixed(_)=> {
                self.enable_fixed_pmc(self.get_pmc_index())
            }
        };
        Ok(())
    }

    fn stop(&self) -> Result<(), ErrorMsg> {
        match self.get_counter_type(){
            Counter::Programmable(_)=>{
                self.disable_general_pmc(self.get_pmc_index())
            }
            Counter::Fixed(_)=> {
                self.disable_fixed_pmc(self.get_pmc_index())
            }
        };
        Ok(())
    }

    fn read(&mut self) -> Result<u64, ErrorMsg> {
        match self.get_counter_type(){
            Counter::Programmable(_)=>{
                return Ok(self.read_general_pmc_ctr(self.get_pmc_index()))
            }
            Counter::Fixed(_)=> {
                return Ok(self.read_fixed_pmc_ctr(self.get_pmc_index()))
            }
        };
    }
}
