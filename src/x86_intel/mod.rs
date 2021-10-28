//! Performance counter for a single progammeable or fixed PMC.
//! 
//! Must have a PerfCounterControler instance.
//! 
//! Have two general usages:
//! 1. Record and Read the occurance of a hardware event through reset() start() read() and stop()
//! 2. Generate a Performance Monitoring Interrupt (PMI) when hitting a certain number of the hardware event through 
//!     globle_ctrl.register_overflow_interrput(), overflow_after() globle_ctrl.get_overflow_counter(), reset() and globle_ctrl.reset_overflow_interrput().
use crate::AbstractPerfCounter;
pub mod globle_ctrl;
use globle_ctrl::PerfCounterControler;
use x86::{msr::*, perfcnt::intel::{ EventDescription,Counter,Tuple}};
pub const ENABLE_GENERAL_PMC_MASK: u64 = 0x1<<22;

#[derive(Debug)]
pub enum ErrorMsg {
    CounterInUse,
    UnsupportedEvent,
    CounterOutOfRange,
    UnsupportedFixPMC,
    UnsupportedVersion,
}


pub struct PerfCounter{
    pub global_ctrler: &'static PerfCounterControler,
    pub counter_type:Counter,
    pub pmc_index:u8,  
    pub general_pmc_mask:u64,
    pub fixed_pmc_mask:u64,
}



impl  PerfCounter{
    pub fn new(global_ctrler: &'static PerfCounterControler) -> PerfCounter{
        PerfCounter{
            global_ctrler: global_ctrler,
            pmc_index: 0,
            counter_type: Counter::Programmable(0),
            general_pmc_mask: 0,
            fixed_pmc_mask: 0,
        }
    }

    ///Build a PerfCounter for one pmc_msr (programmable or fixed) from x86::perfcnt::intel::description
    /// 
    ///Arg index indicates the index of programmable pmc_msr intended to use. It is not used when using fixed_pmc--can input any value.
    pub fn build_from_intel_hw_event(&mut self,event:&EventDescription,index:u8,)->Result<(),ErrorMsg>{
        match event.counter{

            Counter::Fixed(index)=> 
            if self.global_ctrler.get_version_identifier()<2 {
                return Err(ErrorMsg::UnsupportedFixPMC);
            }else if index > self.global_ctrler.get_number_fixed_function_counter(){
                    return Err(ErrorMsg::CounterOutOfRange);
            }else{
                self.counter_type = Counter::Fixed(index);
                self.fixed_pmc_mask= 1<<3 + 3;
                self.pmc_index = index;
                if event.any_thread && self.global_ctrler.get_version_identifier()>2{
                    self.fixed_pmc_mask |= 4;
                }
            }


            Counter::Programmable(_) => 
            if index >= self.global_ctrler.get_number_msr(){
                return Err(ErrorMsg::CounterOutOfRange);
            }else{
                self.pmc_index = index;
                self.counter_type = Counter::Programmable(index);
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
            config |= 1<<17;
            config |= 1<<16;
            config |= 1<<20;
            self.general_pmc_mask = config | ENABLE_GENERAL_PMC_MASK;
            
            }
        }
        Ok(())
    }

    ///Build a PerfCounter for one programmable_pmc_ms from raw eventmask and other attributes.
    pub fn build_general_from_raw(&mut self,eventmask:u32,umask:u32,user_enabled:bool,os_enabled:bool,counter_mask:u8,edge_detect:bool,pmc_index:u8){
        self.general_pmc_mask = 0;
        self.general_pmc_mask |= (eventmask & 0xFF)as u64;
        self.general_pmc_mask |= ((umask<<8) & (0xFF<<8)) as u64;
        self.general_pmc_mask |= (((user_enabled as u64) <<16) & (0x1<<16)) as u64;
        self.general_pmc_mask |= (((os_enabled as u64)<<17) & (0x1<<17)) as u64;
        self.general_pmc_mask |= ENABLE_GENERAL_PMC_MASK;
        self.general_pmc_mask |= (counter_mask as u64) << 24;
        self.general_pmc_mask |= if edge_detect {1<<18}else{0};
        self.general_pmc_mask |= 1 << 20;
        self.counter_type = Counter::Programmable(pmc_index);
    }


    ///Counter will not increment in ring 4
    pub fn exnclude_os(&mut self){
        match self.get_counter_type(){
            Counter::Fixed(_) => {
                self.fixed_pmc_mask &= !(1<<0);
            },
            Counter::Programmable(_) =>{
                self.general_pmc_mask &= !(1<<17);
            },
        }
    }

    ///Counter will not increment in ring 0
    pub fn exclude_user(&mut self){
        match self.get_counter_type(){
            Counter::Fixed(_) => {
                self.fixed_pmc_mask &= !(1<<1);
            },
            Counter::Programmable(_) =>{
                self.general_pmc_mask &= !(1<<16);
            },
        }
        //self.general_pmc_mask &= !(1<<16);
    }

    ///Counter will not produce PMI when overflow
    pub fn disable_interrupt(&mut self){
        match self.get_counter_type(){
            Counter::Fixed(_) => {
                self.fixed_pmc_mask &= !(1<<3);
            },
            Counter::Programmable(_) =>{
                self.general_pmc_mask &= !(1<<20);
            },
        }
        //self.general_pmc_mask &= !(1<<20);
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
    pub fn get_fixed_pmc_mask(&self)->u64{
        self.fixed_pmc_mask
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
        
        let reading = ((rax<<32>>32) | rdx<<32) & ((0x1<<self.global_ctrler.get_bit_width())-1);
        // let reading = ((rax<<32>>32) | rdx<<32);
        /*if self.check_overflow(){
            return reading + (0x1 << self.get_bit_width());
        }*/
        reading
        }
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
        
        let reading = ((rax<<32>>32) | rdx<<32) & ((0x1<<self.global_ctrler.get_bit_width_fixed_counter())-1);

        /*if self.check_overflow(){
            return reading + (0x1 << self.get_bit_width());
        }*/
        reading
        }
    }

    pub fn set_general_pmc_ctr(&self, index:u8,value:u64){
        let value = value & ((1<<self.global_ctrler.get_bit_width()) - 1);
        unsafe {wrmsr(IA32_A_PMC0+index as u32, value)}
    }

    pub fn set_general_pmc_ctrl(&self, mask:u64,index:u8){
        unsafe {wrmsr(0x186+index as u32, mask)}
    }

    pub fn set_fixed_pmc_ctr(&self, index:u8,value:u64){
        let value = value & ((1<<self.global_ctrler.get_bit_width_fixed_counter()) - 1);
        unsafe {wrmsr(0x309+index as u32, value)}
    }

    pub fn set_fixed_pmc_ctrl(&self, index:u8,enabled_ring_lv:u8,is_pmi_enabled:bool){
        let rcx:u32 = 0x38D ;
        let rax:u64 = ((if is_pmi_enabled {8+enabled_ring_lv} else {enabled_ring_lv}) << (index * 4)) as u64; 
       unsafe{ wrmsr(rcx, rax)}
    }

    pub fn enable_general_pmc(&self,index:u8){
        unsafe {
        /*if self.global_ctrler.get_version_identifier()>=2{
            let rcx:u64 = 0x38f;
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
            
        }*/
        self.global_ctrler.enable_counter(self.counter_type);
        wrmsr(0x186+index as u32, self.get_general_pmc_mask());
        }
    }

    pub fn disable_general_pmc(&self,index:u8){
        unsafe{
        /*if self.global_ctrler.get_version_identifier()>=2{
            let rcx:u64 = 0x38f;
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
        }*/
        self.global_ctrler.disable_counter(self.counter_type);
        wrmsr(0x186+index as u32, 0);
        }
    }

    pub fn enable_fixed_pmc(&self,index:u8){
        /*let rcx:u64 = 0x38f;
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
        }*/
        self.global_ctrler.enable_counter(self.counter_type);
        let rcx:u32 = 0x38D ;
        let rax:u64 = (self.get_fixed_pmc_mask()<< (index * 4)) as u64 ; 
       unsafe{ wrmsr(rcx as u32, rdmsr(rcx) &(!(15<<(index*4)))|rax);}
    }

    pub fn disable_fixed_pmc(&self,index:u8){
  
        /*      let rcx:u64 = 0x38f;
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
        }*/
        self.global_ctrler.disable_counter(self.counter_type);
        let rcx:u32 = 0x38D ;
        //let rax:u64 = ((if self.get_is_pmc_pmi_enabled() {8+self.get_fixed_pmc_ring_lv()} else {self.get_fixed_pmc_ring_lv()}) << (index * 4)) as u64; 
       unsafe{ wrmsr(rcx, rdmsr(rcx) & (15<<(index * 4)));}
    }

    pub fn check_overflow(&self)->bool{
        match self.get_counter_type(){
            Counter::Programmable(_) => {
                return self.global_ctrler.read_overflow_status() & (0x1 << self.get_pmc_index()) > 0
                
            }
            Counter::Fixed(_) => {
                return self.global_ctrler.read_overflow_status() & (0x1 << (self.get_pmc_index()+32)) > 0
            }
        }
    }


    pub fn overflow_after(&self,value:u64){
        match self.get_counter_type(){
            Counter::Fixed(_) => {
                self.set_fixed_pmc_ctr(self.get_pmc_index(), !value);
            },
            Counter::Programmable(_) => {
                self.set_general_pmc_ctr(self.get_pmc_index(), !value);
            }
        }
    }
}

impl<'a> AbstractPerfCounter for PerfCounter {
    fn reset(&self) -> Result<(),ErrorMsg> {
        match self.get_counter_type(){
            Counter::Programmable(_) => self.set_general_pmc_ctr(self.get_pmc_index(),0),
            Counter::Fixed(_) => self.set_fixed_pmc_ctr(self.get_pmc_index(),0),
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
