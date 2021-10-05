use crate::AbstractPerfCounter;

pub enum ErrorMsg {
    CounerInUse,
    UnsupportedEvent,
}

pub struct PerfCounter{
    version_identifier:u8,
    number_msr:u8,
    number_fixed_function_counter:u8,
    bit_width:u8,
    events_available:u8,
}

impl  PerfCounter{
    pub fn new()->PerfCounter{
            let mut rax :u64;
            let mut rbx:u64;
        unsafe{
            //get CPU info
            asm!(
                "MOV EAX, 0AH",
                "CPUID",
                //"mov rdx, rbx",
                out("rax") rax,
                out("rdx") rbx,
            );
           
        } 
        let mask:u64 =  255;
        PerfCounter{
            version_identifier : (rax & mask) as u8,
            number_msr : ((rax >> 8) & mask) as u8,
            bit_width :  ((rax >> 16) & mask) as u8,
            events_available : ((rax >> 24) & mask )as u8,
            number_fixed_function_counter : (rbx & 63 )as u8,
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

    pub fn read_general_pmc(&self, index:u8)->u64{
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

    pub fn set_general_pmc(&self, mask:u64,index:u8){
        let rcx:u64 = 0x186 + (index as u64);
        let rax:u64 = mask;
        let rdx:u64 = mask>>32;
        unsafe{
            asm!(
                "wrmsr",
                in("rcx") rcx,
                in("rax") rax,
                in("rdx") rdx,
            )
        }

    }

    pub fn enable_pmc(&self,index:u64){
        let rcx:u64 = 0x38f+index;
        let rax:u64 = 1;
        let rdx:u64 = 0;
        unsafe{
            asm!(
                "wrmsr",
                in("rcx") rcx,
                in("rax") rax,
                in("rdx") rdx,
            )
        }
    }
}

impl<'a> AbstractPerfCounter for PerfCounter {
    fn reset(&self) -> Result<(),ErrorMsg> {
        Ok(())
    }

    fn start(&self) -> Result<(), ErrorMsg> {
        Ok(())
    }

    fn stop(&self) -> Result<(), ErrorMsg> {
        Ok(())
    }

    fn read(&mut self) -> Result<u64, ErrorMsg> {
        return Ok(0);
    }
}
