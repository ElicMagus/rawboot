use crate::{
    io::{read32, sdelay, write32},
    print,
};
#[repr(C)]
#[derive(Clone, Copy)]
struct DramPara_t {
    //normal configuration
    dram_clk: u32,
    dram_type: u32, //dram_type			DDR2: 2				DDR3: 3		LPDDR2: 6	LPDDR3: 7	DDR3L: 31
    //unsigned int        lpddr2_type,	//LPDDR2 type		S4:0	S2:1 	NVM:2
    dram_zq: u32, //do not need
    dram_odt_en: u32,
    //control configuration
    dram_para1: u32,
    dram_para2: u32,
    //timing configuration
    dram_mr0: u32,
    dram_mr1: u32,
    dram_mr2: u32,
    dram_mr3: u32,
    dram_tpr0: u32, //DRAMTMG0
    dram_tpr1: u32, //DRAMTMG1
    dram_tpr2: u32, //DRAMTMG2
    dram_tpr3: u32, //DRAMTMG3
    dram_tpr4: u32, //DRAMTMG4
    dram_tpr5: u32, //DRAMTMG5
    dram_tpr6: u32, //DRAMTMG8
    //reserved for future use
    dram_tpr7: u32,
    dram_tpr8: u32,
    dram_tpr9: u32,
    dram_tpr10: u32,
    dram_tpr11: u32,
    dram_tpr12: u32,
    dram_tpr13: u32,
}

impl DramPara_t {
    pub fn init(self) -> u32 {
        let mut rc: u32;
        let mut mem_size: u32;
        // Test ZQ status
        if self.dram_tpr13 & 0x10000 != 0 {
            print!("DRAM only have internal ZQ!!\r\n");
            write32(0x3000160, read32(0x3000160) | 0x100);
            write32(0x3000168, 0);
            sdelay(10);
        } else {
            write32(0x3000160, read32(0x3000160) & 0xfffffffc);
            write32(0x7010254, self.dram_tpr13 & 0x10000);
            sdelay(10);
            write32(0x3000160, (read32(0x3000160) & 0xfffffef7) | 2);
            sdelay(10);
            write32(0x3000160, read32(0x3000160) | 0x001);
            sdelay(20);
            print!("ZQ value = 0x{:x}\r\n", read32(0x300016c));
        }

        // Set voltage
        self.vol_set();

        // Set SDRAM controller auto config
        if (self.dram_tpr13 & 0x1) == 0 {
            if self.auto_scan_dram_config() == 0 {
                print!("auto_scan_dram_config() FAILED\r\n");
                return 0;
            }
        }

        // Print header message (too late)
        print!("DRAM BOOT DRIVE INFO: {:?}\r\n", "V0.24");
        print!("DRAM CLK = {:?} MHz\r\n", self.dram_clk);
        print!("DRAM Type = {:?} (2:DDR2,3:DDR3)\r\n", self.dram_type);
        if (self.dram_odt_en & 0x1) == 0 {
            print!("DRAMC read ODT  off.\r\n");
        } else {
            print!("DRAMC ZQ value: 0x{:x?}\r\n", self.dram_zq);
        }

        // report ODT
        rc = self.dram_mr1;
        if (rc & 0x44) == 0 {
            print!("DRAM ODT off.\r\n");
        } else {
            print!("DRAM ODT value: 0x{:x?}.\r\n", rc);
        }

        // Init core, final run
        if (self.mctl_core_init() == 0) {
            print!("DRAM initialisation error : 1 !\r\n");
            return 0;
        }

        // Get sdram size
        rc = self.dram_para2;
        if rc < 0 {
            rc = (rc & 0x7fff0000u32) >> 16;
        } else {
            rc = self.get_size();
            print!("DRAM SIZE = {:?}M\r\n", rc);
            // self.dram_para2 = (self.dram_para2 & 0xffffu32) | rc << 16;
        }
        mem_size = rc;

        // #if 0
        //     // Purpose ??
        //     if (self.dram_tpr13 & (1 << 30)) {
        //         rc = read32(self.dram_tpr8);
        //         if (rc == 0) {
        //             rc = 0x10000200;
        //         }
        //         write32(0x31030a0, rc);
        //         write32(0x310309c, 0x40a);
        //         write32(0x3103004, read32(0x3103004) | 1);
        //         print!("Enable Auto SR");
        //     } else {
        // #endif
        write32(0x31030a0, read32(0x31030a0) & 0xffff0000);
        write32(0x3103004, read32(0x3103004) & (!0x1));
        //	}

        // Pupose ??
        rc = read32(0x3103100) & !(0xf000);
        if (self.dram_tpr13 & 0x200) == 0 {
            if (self.dram_type != 6) {
                write32(0x3103100, rc);
            }
        } else {
            write32(0x3103100, rc | 0x5000);
        }

        write32(0x3103140, read32(0x3103140) | (1 << 31));
        if (self.dram_tpr13 & (1 << 8)) != 0 {
            write32(0x31030b8, read32(0x3103140) | 0x300);
        }

        rc = read32(0x3103108);
        if (self.dram_tpr13 & (1 << 16)) != 0 {
            rc &= 0xffffdfff;
        } else {
            rc |= 0x00002000;
        }
        write32(0x3103108, rc);

        // Purpose ??
        if self.dram_type == 7 {
            rc = read32(0x310307c) & 0xfff0ffff;
            rc |= 0x0001000;
            write32(0x310307c, rc);
        }

        self.enable_all_master();
        if (self.dram_tpr13 & (1 << 28)) != 0 {
            rc = read32(0x70005d4);
            // if ((rc & (1 << 16))!=0 || dramc_simple_wr_test(mem_size, 4096))!=0 {
            //     return 0;
            // }
        }
        mem_size
    }
    fn vol_set(self) {
        let mut reg = 0;
        let mut vol = 0;

        match self.dram_type {
            2 => {
                vol = 47;
            }

            3 => {
                vol = 25;
            }

            _ => {
                vol = 0;
            }
        }

        vol = 25; // XXX

        reg = read32(0x3000150);
        reg = (reg & 0xffdf00ff) | vol << 8;
        /*
            reg = read32(0x3000150);
            reg &= ~(0xff00);
            reg |= vol << 8;
            reg &= ~(0x200000);
        */
        write32(0x3000150, reg);

        sdelay(1);

        // sid_read_ldoB_cal(para);
    }

    fn auto_scan_dram_config(self) -> u32 {
        return 0u32;
    }
    fn mctl_core_init(self) -> u32 {
        return 0u32;
    }
    fn get_size(self) -> u32 {
        return 0u32;
    }
    fn enable_all_master(self) {}

    fn dramc_simple_wr_test(mem_size: u32, test: u32) -> u32 {
        0u32
    }
}
