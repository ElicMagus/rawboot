use crate::{
    io::{read32, sdelay, write32},
    print,
};

const RAM_BASE: u32 = 0x40000000;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct DramPara_t {
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

impl DramPara_t 
{
    pub fn init(mut self) -> u32 {
        let mut rc: u32;
        let mem_size: u32;
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
        // if self.mctl_core_init() == 0 {
            print!("DRAM initialisation error : 1 !\r\n");
            return 0;
        // }

        // Get sdram size
        rc = self.dram_para2;
        if rc < 0 {
            rc = (rc & 0x7fff0000u32) >> 16;
        } else {
            // rc = self.get_size();
            print!("DRAM SIZE = {:?}M\r\n", rc);
            self.dram_para2 = (self.dram_para2 & 0xffffu32) | rc << 16;
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
            if self.dram_type != 6 {
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

        // self.enable_all_master();
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

        self.sid_read_ldo_b_cal();
    }

    fn sid_read_ldo_b_cal(self) {
        let mut reg: u32;

        reg = (read32(0x0300621c) << 0x10) >> 0x18;

        if reg != 0 {
            if self.dram_type != 2 {
                if self.dram_type == 3 {
                    if 0x20 < reg {
                        reg = reg - 0x16;
                    }
                } else {
                    reg = 0;
                }
            }
            write32(0x03000150, (read32(0x03000150) & 0xffff00ff) | (reg << 8));
        }
    }

    fn auto_scan_dram_config(mut self) -> u32 {
        if ((self.dram_tpr13 & (1 << 14)) == 0) && (self.auto_scan_dram_rank_width() == 0) {
            print!("[ERROR DEBUG] auto scan dram rank & width failed !\r\n");
            return 0;
        }
        if ((self.dram_tpr13 & (1 << 0)) == 0) && (self.auto_scan_dram_size() == 0) {
            print!("[ERROR DEBUG] auto scan dram size failed !\r\n");
            return 0;
        }
        if (self.dram_tpr13 & (1 << 15)) == 0 {
            self.dram_tpr13 |= 0x6003;
        }
        return 1;
    }

    // Autoscan sizes a dram device by cycling through address lines and figuring
    // out if it is connected to a real address line, or if the address is a mirror.
    // First the column and bank bit allocations are set to low values (2 and 9 address
    // lines. Then a maximum allocation (16 lines) is set for rows and this is tested.
    // Next the BA2 line is checked. This seems to be placed above the column, BA0-1 and
    // row addresses. Finally, the column address is allocated 13 lines and these are
    // tested. The results are placed in dram_para1 and dram_para2.
    //
    fn auto_scan_dram_size(mut self) -> u32 {
        let mut rval: u32 = 0;
        let mut i = 0;
        let mut j: u32 = 0;
        let mut rank = 0;
        let mut maxrank = 0;
        let mut offs = 0;
        let mut mc_work_mode = 0;
        let mut chk = 0;
        let mut ptr = 0;
        let mut shft = 0;
        // #ifdef CONFIG_DEBUG_DDR_DRIVER
        let mut banks = 0;
        // #endif

        if self.mctl_core_init() == 0 {
            print!("[ERROR DEBUG] DRAM initialisation error : 0!\r\n");
            return 0;
        }

        maxrank = if (self.dram_para2 & 0xf000) != 0 {
            2
        } else {
            1
        };
        mc_work_mode = 0x3102000;
        offs = 0;

        // write test pattern
        // for (i = 0, ptr = RAM_BASE; i < 64; i++, ptr += 4) {
        // 	write32(ptr, (i & 1) ? ptr : ~ptr);
        // }
        rank = 0;
        while rank < maxrank {
            // Set row mode
            rval = read32(mc_work_mode);
            rval &= 0xfffff0f3;
            rval |= 0x000006f0;
            write32(mc_work_mode, rval);
            while read32(mc_work_mode) != rval {
                print!("read\r\n");
            }

            // Scan per address line, until address wraps (i.e. see shadow)
            for i in 11..17 {
                chk = RAM_BASE + (1 << (i + 11));
                ptr = RAM_BASE;

                for j in 0..64 {
                    if read32(chk) != (if j & 1 != 0 { ptr } else { !ptr }) {
                        break;
                    } else {
                        ptr += 4;
                        chk += 4;
                    }
                }
            }

            if i > 16 {
                i = 16;
            }

            print!("[AUTO DEBUG] rank {:?} row = {:?}\r\n", rank, i);

            // Store rows in para 1
            shft = 4 + offs;
            rval = self.dram_para1;
            rval &= !(0xff << shft);
            rval |= i << shft;
            self.dram_para1 = rval;

            if rank == 1 {
                // Set bank mode for rank0
                rval = read32(0x3102000);
                rval &= 0xfffff003;
                rval |= 0x000006a4;
                write32(0x3102000, rval);
            }

            // Set bank mode for current rank
            rval = read32(mc_work_mode);
            rval &= 0xfffff003;
            rval |= 0x000006a4;
            write32(mc_work_mode, rval);
            while read32(mc_work_mode) != rval {}

            // Test if bit A23 is BA2 or mirror XXX A22?
            chk = RAM_BASE + (1 << 22);
            ptr = RAM_BASE;
            j = 0;
            for _i in 0..64 {
                if read32(chk) != (if j & 1 != 0 { ptr } else { !ptr }) {
                    j = 1;
                    break;
                }
                ptr += 4;
                chk += 4;
            }

            // #ifdef CONFIG_DEBUG_DDR_DRIVER
            banks = (j + 1) << 2; // 4 or 8
                                  // #endif
            print!("[AUTO DEBUG] rank {:?} bank = {:?}\r\n", rank, banks);

            // Store banks in para 1
            shft = 12 + offs;
            rval = self.dram_para1;
            rval &= !(0xf << shft);
            rval |= j << shft;
            self.dram_para1 = rval;

            if rank == 1 {
                // Set page mode for rank0
                rval = read32(0x3102000);
                rval &= 0xfffff003;
                rval |= 0x00000aa0;
                write32(0x3102000, rval);
            }

            // Set page mode for current rank
            rval = read32(mc_work_mode);
            rval &= 0xfffff003;
            rval |= 0x00000aa0;
            write32(mc_work_mode, rval);
            while read32(mc_work_mode) != rval {}

            // Scan per address line, until address wraps (i.e. see shadow)
            for i in 9..14 {
                chk = RAM_BASE + (1 << i);
                ptr = RAM_BASE;
                for j in 0..64 {
                    if read32(chk) != (if j & 1 != 0 { ptr } else { !ptr }) {
                        break;
                    } else {
                        ptr += 4;
                        chk += 4;
                    }
                }
            }
            if i > 13 {
                i = 13;
            }

            let pgsize = if i == 9 { 0 } else { 1 << (i - 10) };
            print!(
                "[AUTO DEBUG] rank {:?} page size = {:?} KB\r\n",
                rank, pgsize
            );

            // Store page size
            shft = offs;
            rval = self.dram_para1;
            rval &= !(0xf << shft);
            rval |= pgsize << shft;
            self.dram_para1 = rval;

            // Move to next rank
            rank += 1;
            if rank != maxrank {
                if rank == 1 {
                    rval = read32(0x3202000); // MC_WORK_MODE
                    rval &= 0xfffff003;
                    rval |= 0x000006f0;
                    write32(0x3202000, rval);

                    rval = read32(0x3202004); // MC_WORK_MODE2
                    rval &= 0xfffff003;
                    rval |= 0x000006f0;
                    write32(0x3202004, rval);
                }
                offs += 16; // store rank1 config in upper half of para1
                mc_work_mode += 4; // move to MC_WORK_MODE2
            }
        }
        if maxrank == 2 {
            self.dram_para2 &= 0xfffff0ff;
            // note: rval is equal to self.dram_para1 here
            if (rval & 0xffff) == ((rval >> 16) & 0xffff) {
                print!("rank1 config same as rank0\r\n");
            } else {
                self.dram_para2 |= 0x00000100;
                print!("rank1 config different from rank0\r\n");
            }
        }
        return 1;
    }

    // This routine sets up parameters with dqs_gating_mode equal to 1 and two
    // ranks enabled. It then configures the core and tests for 1 or 2 ranks and
    // full or half DQ width. it then resets the parameters to the original values.
    // dram_para2 is updated with the rank & width findings.
    //
    fn auto_scan_dram_rank_width(mut self) -> u32 {
        let s1: u32 = self.dram_tpr13;
        let s2: u32 = self.dram_para1;
        let mut v: u32;

        self.dram_para1 = 0x00b000b0;
        v = (self.dram_para2 & 0xfffffff0) | 0x1000;
        self.dram_para2 = v;

        v = (s1 & 0xfffffff7) | 0x5; // set DQS probe mode
        self.dram_tpr13 = v;

        self.mctl_core_init();

        if (read32(0x3103010) & (1 << 20)) != 0 {
            return 0;
        }

        if self.dqs_gate_detect() == 0 {
            return 0;
        }

        self.dram_tpr13 = s1;
        self.dram_para1 = s2;
        return 1;
    }

    // The below routine reads the command status register to extract
    // DQ width and rank count. This follows the DQS training command in
    // channel_init. If error bit 22 is reset, we have two ranks and full DQ.
    // If there was an error, figure out whether it was half DQ, single rank,
    // or both. Set bit 12 and 0 in dram_para2 with the results.
    //
    fn dqs_gate_detect(mut self) -> u32 {
        let mut u1: u32;
        let u2: u32;

        if (read32(0x3103010) & (1 << 22)) != 0 {
            u1 = (read32(0x03103348) << 6) >> 0x1e;
            u2 = (read32(0x031033c8) << 6) >> 0x1e;

            if u1 == 2 {
                u1 = self.dram_para2 & 0xffff0ff0;

                if u2 == 2 {
                    self.dram_para2 = u1;
                    print!("[AUTO DEBUG] single rank and full DQ!\r\n");
                } else {
                    self.dram_para2 = u1 | 1;
                    print!("[AUTO DEBUG] single rank and half DQ!\r\n");
                }
            } else {
                if u1 != 0 {
                    if (self.dram_tpr13 & 0x20000000) == 0 {
                        return 0;
                    }

                    print!("DX0 state: {:?}\r\n", u1);
                    print!("DX1 state: {:?}\r\n", u2);
                    return 0;
                }

                self.dram_para2 = (self.dram_para2 & 0xfffffff0) | 0x1001;
                print!("[AUTO DEBUG] dual rank and half DQ!\r\n");
            }
        } else {
            self.dram_para2 = (self.dram_para2 & 0xfffffff0) | 0x1000;
            print!("[AUTO DEBUG] two rank and full DQ!\r\n");
        }

        return 1;
    }

    fn mctl_core_init(self) -> u32 {
        self.mctl_sys_init();

        self.mctl_vrefzq_init();

        self.mctl_com_init();

        self.mctl_phy_ac_remapping();

        self.auto_set_timing_para();

        let res = self.mctl_channel_init(0);
        return res;
    }

    // Init the controller channel. The key part is placing commands in the main
    // command register (PIR, 0x3103000) and checking command status (PGSR0, 0x3103010).
    //
    fn mctl_channel_init(mut self, ch_index: u32) -> u32 {
        let mut val: u32 = 0;
        let mut dqs_gating_mode: u32 = 0;

        dqs_gating_mode = (self.dram_tpr13 >> 2) & 0x3;

        // set DDR clock to half of CPU clock
        val = read32(0x310200c) & 0xfffff000;
        val |= (self.dram_clk >> 1) - 1;
        write32(0x310200c, val);

        // MRCTRL0 nibble 3 undocumented
        val = read32(0x3103108) & 0xfffff0ff;
        val |= 0x300;
        write32(0x3103108, val);

        val = ((!self.dram_odt_en) & 1) << 5;

        // DX0GCR0
        if self.dram_clk > 672 {
            write32(0x3103344, read32(0x3103344) & 0xffff09c1);
        } else {
            write32(0x3103344, read32(0x3103344) & 0xffff0fc1);
        }
        write32(0x3103344, read32(0x3103344) | val);

        // DX1GCR0
        if self.dram_clk > 672 {
            write32(0x3103344, read32(0x3103344) | 0x400);
            write32(0x31033c4, read32(0x31033c4) & 0xffff09c1);
        } else {
            write32(0x31033c4, read32(0x31033c4) & 0xffff0fc1);
        }
        write32(0x31033c4, read32(0x31033c4) | val);

        // 0x3103208 undocumented
        write32(0x3103208, read32(0x3103208) | 2);

        // eye_delay_compensation(para);

        // set PLL SSCG ?
        //
        val = read32(0x3103108);
        if dqs_gating_mode == 1 {
            val &= !(0xc0);
            write32(0x3103108, val);

            val = read32(0x31030bc);
            val &= 0xfffffef8;
            write32(0x31030bc, val);
        } else if dqs_gating_mode == 2 {
            val &= !(0xc0);
            val |= 0x80;
            write32(0x3103108, val);

            val = read32(0x31030bc);
            val &= 0xfffffef8;
            val |= ((self.dram_tpr13 >> 16) & 0x1f) - 2;
            val |= 0x100;
            write32(0x31030bc, val);

            val = read32(0x310311c) & 0x7fffffff;
            val |= 0x08000000;
            write32(0x310311c, val);
        } else {
            val &= !(0x40);
            write32(0x3103108, val);

            sdelay(10);

            val = read32(0x3103108);
            val |= 0xc0;
            write32(0x3103108, val);
        }

        if self.dram_type == 6 || self.dram_type == 7 {
            val = read32(0x310311c);
            if dqs_gating_mode == 1 {
                val &= 0xf7ffff3f;
                val |= 0x80000000;
            } else {
                val &= 0x88ffffff;
                val |= 0x22000000;
            }
            write32(0x310311c, val);
        }

        val = read32(0x31030c0);
        val &= 0xf0000000;
        val |= if (self.dram_para2 & (1 << 12)) != 0 {
            0x03000001
        } else {
            0x01000007
        }; // 0x01003087 XXX
        write32(0x31030c0, val);

        if (read32(0x70005d4) & (1 << 16)) != 0 {
            val = read32(0x7010250);
            val &= 0xfffffffd;
            write32(0x7010250, val);

            sdelay(10);
        }

        // Set ZQ config
        val = read32(0x3103140) & 0xfc000000;
        val |= self.dram_zq & 0x00ffffff;
        val |= 0x02000000;
        write32(0x3103140, val);

        // Initialise DRAM controller
        if dqs_gating_mode == 1 {
            //		write32(0x3103000, 0x52); // prep PHY reset + PLL init + z-cal
            write32(0x3103000, 0x53); // Go

            while (read32(0x3103010) & 0x1) == 0 {} // wait for IDONE
            sdelay(10);

            // 0x520 = prep DQS gating + DRAM init + d-cal
            //		val = (self.dram_type == 3) ? 0x5a0	// + DRAM reset
            //					     : 0x520;

            if self.dram_type == 3 {
                write32(0x3103000, 0x5a0);
            } else {
                write32(0x3103000, 0x520);
            }
        } else {
            if (read32(0x70005d4) & (1 << 16)) == 0 {
                // prep DRAM init + PHY reset + d-cal + PLL init + z-cal
                //			val = (self.dram_type == 3) ? 0x1f2	// + DRAM reset
                //						     : 0x172;

                if self.dram_type == 3 {
                    write32(0x3103000, 0x1f2);
                } else {
                    write32(0x3103000, 0x172);
                }
            } else {
                // prep PHY reset + d-cal + z-cal
                //			val = 0x62;
                write32(0x3103000, 0x62);
            }
        }

        //	write32(0x3103000, val); // Prep
        //	val |= 1;
        //	write32(0x3103000, val); // Go

        write32(0x3103000, read32(0x3103000) | 1); // GO

        sdelay(10);
        while (read32(0x3103010) & 0x1) == 0 {} // wait for IDONE

        if (read32(0x70005d4) & (1 << 16)) != 0 {
            val = read32(0x310310c);
            val &= 0xf9ffffff;
            val |= 0x04000000;
            write32(0x310310c, val);

            sdelay(10);

            val = read32(0x3103004);
            val |= 0x1;
            write32(0x3103004, val);

            while (read32(0x3103018) & 0x7) != 0x3 {}

            val = read32(0x7010250);
            val &= 0xfffffffe;
            write32(0x7010250, val);

            sdelay(10);

            val = read32(0x3103004);
            val &= 0xfffffffe;
            write32(0x3103004, val);

            while (read32(0x3103018) & 0x7) != 0x1 {}

            sdelay(15);

            if dqs_gating_mode == 1 {
                val = read32(0x3103108);
                val &= 0xffffff3f;
                write32(0x3103108, val);

                val = read32(0x310310c);
                val &= 0xf9ffffff;
                val |= 0x02000000;
                write32(0x310310c, val);

                sdelay(1);
                write32(0x3103000, 0x401);

                while (read32(0x3103010) & 0x1) == 0 {}
            }
        }

        // Check for training error
        /*	val = read32(0x3103010);
            if (((val >> 20) & 0xff) && (val & 0x100000)) {
                ddr_debug("ZQ calibration error, check external 240 ohm resistor.\r\n");
                return 0;
            }
        */
        if (read32(0x3103010) & 0xff00000) == 0 {
            val = 1;
        } else {
            val = read32(0x3103010) & 0x100000;

            if val != 0 {
                print!("ZQ calibration error, check external 240 ohm resistor.\r\n");
                return 0;
            }
        }

        // STATR = Zynq STAT? Wait for status 'normal'?
        while (read32(0x3103018) & 0x1) == 0 {}

        val = read32(0x310308c);
        val |= 0x80000000;
        write32(0x310308c, val);

        sdelay(10);

        val = read32(0x310308c);
        val &= 0x7fffffff;
        write32(0x310308c, val);

        sdelay(10);

        val = read32(0x3102014);
        val |= 0x80000000;
        write32(0x3102014, val);

        sdelay(10);

        val = read32(0x310310c);
        val &= 0xf9ffffff;
        write32(0x310310c, val);

        if dqs_gating_mode == 1 {
            val = read32(0x310311c);
            val &= 0xffffff3f;
            val |= 0x00000040;
            write32(0x310311c, val);
        }

        return 1;
    }

    // Main purpose of the auto_set_timing routine seems to be to calculate all
    // timing settings for the specific type of sdram used. Read together with
    // an sdram datasheet for context on the various variables.
    //
    fn auto_set_timing_para(mut self) // s5
    {
        let mut freq: u32 = 0; // s4
        let mut vtype: u32 = 0; // s8
        let mut tpr13: u32 = 0; // 80(sp)
        let mut reg_val: u32 = 0;

        let mut tccd: u8 = 0; // 88(sp)
        let mut trrd: u8 = 0; // s7
        let mut trcd: u8 = 0; // s3
        let mut trc: u8 = 0; // s9
        let mut tfaw: u8 = 0; // s10
        let mut tras: u8 = 0; // s11
        let mut trp: u8 = 0; // 0(sp)
        let mut twtr: u8 = 0; // s1
        let mut twr: u8 = 0; // s6
        let mut trtp: u8 = 0; // 64(sp)
        let mut txp: u8 = 0; // a6
        let mut trefi: u16 = 0; // s2
        let mut trfc: u16 = 0; // a5 / 8(sp)

        freq = self.dram_clk;
        vtype = self.dram_type;
        tpr13 = self.dram_tpr13;

        // ddr_debug("vtype = %d\r\n", type);
        // ddr_debug("tpr13 = %p\r\n", tpr13);

        if (self.dram_tpr13 & 0x2) != 0 {
            // dram_tpr0
            tccd = ((self.dram_tpr0 >> 21) & 0x7) as u8; // [23:21]
            tfaw = ((self.dram_tpr0 >> 15) & 0x3f) as u8; // [20:15]
            trrd = ((self.dram_tpr0 >> 11) & 0xf) as u8; // [14:11]
            trcd = ((self.dram_tpr0 >> 6) & 0x1f) as u8; // [10:6 ]
            trc = ((self.dram_tpr0 >> 0) & 0x3f) as u8; // [ 5:0 ]
                                                        // dram_tpr1
            txp = ((self.dram_tpr1 >> 23) & 0x1f) as u8; // [27:23]
            twtr = ((self.dram_tpr1 >> 20) & 0x7) as u8; // [22:20]
            trtp = ((self.dram_tpr1 >> 15) & 0x1f) as u8; // [19:15]
            twr = ((self.dram_tpr1 >> 11) & 0xf) as u8; // [14:11]
            trp = ((self.dram_tpr1 >> 6) & 0x1f) as u8; // [10:6 ]
            tras = ((self.dram_tpr1 >> 0) & 0x3f) as u8; // [ 5:0 ]
                                                         // dram_tpr2
            trfc = ((self.dram_tpr2 >> 12) & 0x1ff) as u16; // [20:12]
            trefi = ((self.dram_tpr2 >> 0) & 0xfff) as u16; // [11:0 ]
        } else {

            let mut frq2 = freq >> 1; // s0

            if vtype == 3 {
                // DDR3
                // trfc  = auto_cal_timing(350, frq2);
                // trefi = auto_cal_timing(7800, frq2) / 32 + 1; // XXX
                // twr	  = auto_cal_timing(8, frq2);
                // trcd  = auto_cal_timing(15, frq2);
                twtr = twr + 2; // + 2 ? XXX
                if twr < 2 {
                    twtr = 2;
                }
                twr = trcd;
                if trcd < 2 {
                    twr = 2;
                }
                if freq <= 800 {
                    // tfaw = auto_cal_timing(50, frq2);
                    // trrd = auto_cal_timing(10, frq2);
                    if trrd < 2 {
                        trrd = 2;
                    }
                    // trc	 = auto_cal_timing(53, frq2);
                    // tras = auto_cal_timing(38, frq2);
                    txp = trrd; // 10
                    trp = trcd; // 15
                } else {
                    // tfaw = auto_cal_timing(35, frq2);
                    // trrd = auto_cal_timing(10, frq2);
                    if trrd < 2 {
                        trrd = 2;
                    }
                    // trcd = auto_cal_timing(14, frq2);
                    // trc	 = auto_cal_timing(48, frq2);
                    // tras = auto_cal_timing(34, frq2);
                    txp = trrd; // 10
                    trp = trcd; // 14
                }
            // #if 1
            } else if vtype == 2 {
                // DDR2
                // tfaw  = auto_cal_timing(50, frq2);
                // trrd  = auto_cal_timing(10, frq2);
                // trcd  = auto_cal_timing(20, frq2);
                // trc	  = auto_cal_timing(65, frq2);
                // twtr  = auto_cal_timing(8, frq2);
                // trp	  = auto_cal_timing(15, frq2);
                // tras  = auto_cal_timing(45, frq2);
                // trefi = auto_cal_timing(7800, frq2) / 32;
                // trfc  = auto_cal_timing(328, frq2);
                txp = 2;
                twr = trp; // 15
            } else if vtype == 6 {
                // LPDDR2
                // tfaw = auto_cal_timing(50, frq2);
                if tfaw < 4 {
                    tfaw = 4;
                }
                // trrd = auto_cal_timing(10, frq2);
                if trrd == 0 {
                    trrd = 1;
                }
                // trcd = auto_cal_timing(24, frq2);
                if trcd < 2 {
                    trcd = 2;
                }
                // trc = auto_cal_timing(70, frq2);
                // txp = auto_cal_timing(8, frq2);
                if txp == 0 {
                    txp = 1;
                    twtr = 2;
                } else {
                    twtr = txp;
                    if txp < 2 {
                        txp = 2;
                        twtr = 2;
                    }
                }
                // twr = auto_cal_timing(15, frq2);
                if twr < 2 {
                    twr = 2;
                }
            // trp	  = auto_cal_timing(17, frq2);
            // tras  = auto_cal_timing(42, frq2);
            // trefi = auto_cal_timing(3900, frq2) / 32;
            // trfc  = auto_cal_timing(210, frq2);
            } else if vtype == 7 {
                // LPDDR3
                // tfaw = auto_cal_timing(50, frq2);
                if tfaw < 4 {
                    tfaw = 4;
                }
                // trrd = auto_cal_timing(10, frq2);
                if trrd == 0 {
                    trrd = 1;
                }
                // trcd = auto_cal_timing(24, frq2);
                if trcd < 2 {
                    trcd = 2;
                }
                // trc	 = auto_cal_timing(70, frq2);
                // twtr = auto_cal_timing(8, frq2);
                if twtr < 2 {
                    twtr = 2;
                }
                // twr = auto_cal_timing(15, frq2);
                if twr < 2 {
                    twr = 2;
                }
                // trp	  = auto_cal_timing(17, frq2);
                // tras  = auto_cal_timing(42, frq2);
                // trefi = auto_cal_timing(3900, frq2) / 32;
                // trfc  = auto_cal_timing(210, frq2);
                txp = twtr;
            // #endif
            } else {
                // default
                trfc = 128;
                trp = 6;
                trefi = 98;
                txp = 10;
                twr = 8;
                twtr = 3;
                tras = 14;
                tfaw = 16;
                trc = 20;
                trcd = 6;
                trrd = 3;
            };
            // assign the value back to the DRAM structure
            tccd = 2;
            trtp = 4; // not in .S ?
            self.dram_tpr0 = ((trc as u32) << 0) | ((trcd as u32) << 6) | ((trrd as u32) << 11) | ((tfaw as u32) << 15) | ((tccd as u32) << 21);
            self.dram_tpr1 = ((tras as u32)<< 0)
                | ((trp as u32)<< 6)
                | ((twr as u32) << 11)
                | ((trtp as u32)<< 15)
                | ((twtr as u32) << 20) 
                | ((txp as u32) << 23);
            self.dram_tpr2 = ((trefi as u32) << 0) | ((trfc as u32) << 12);

            let mut tref = (self.dram_tpr4 << 0x10) >> 0x1c;
            if tref == 1 {
                print!("trefi:3.9ms\r\n");
            } else if tref == 2 {
                print!("trefi:1.95ms\r\n");
            } else {
                print!("trefi:7.8ms\r\n");
            }
        }

        let mut tcksrx; // t1
        let mut tckesr; // t4;
        let mut trd2wr; // t6
        let mut trasmax; // t3;
        let mut twtp; // s6 (was twr!)
        let mut tcke; // s8
        let mut tmod; // t0
        let mut tmrd; // t5
        let mut tmrw; // a1
        let mut t_rdata_en; // a4 (was tcwl!)
        let mut tcl; // a0
        let mut wr_latency; // a7
        let mut tcwl; // first a4, then a5
        let mut mr3; // s0
        let mut mr2; // t2
        let mut mr1; // s1
        let mut mr0; // a3
        let mut dmr3; // 72(sp)
                      // unsigned int trtp;	// 64(sp)
        let mut dmr1; // 56(sp)
        let mut twr2rd; // 48(sp)
        let mut tdinit3; // 40(sp)
        let mut tdinit2; // 32(sp)
        let mut tdinit1; // 24(sp)
        let mut tdinit0; // 16(sp)

        dmr1 = self.dram_mr1;
        dmr3 = self.dram_mr3;

        match vtype {
            // #if 1
            2 =>
            // DDR2
            {
                trasmax = freq / 30;
                if freq < 409 {
                    tcl = 3;
                    t_rdata_en = 1;
                    mr0 = 0x06a3;
                } else {
                    t_rdata_en = 2;
                    tcl = 4;
                    mr0 = 0x0e73;
                }
                tmrd = 2;
                twtp = twr + 5;
                tcksrx = 5;
                tckesr = 4;
                trd2wr = 4;
                tcke = 3;
                tmod = 12;
                wr_latency = 1;
                mr3 = 0;
                mr2 = 0;
                tdinit0 = 200 * freq + 1;
                tdinit1 = 100 * freq / 1000 + 1;
                tdinit2 = 200 * freq + 1;
                tdinit3 = 1 * freq + 1;
                tmrw = 0;
                twr2rd = twtr + 5;
                tcwl = 0;
                mr1 = dmr1;
            }
            // #endif
            3 =>
            // DDR3
            {
                trasmax = freq / 30;
                if freq <= 800 {
                    mr0 = 0x1c70;
                    tcl = 6;
                    wr_latency = 2;
                    tcwl = 4;
                    mr2 = 24;
                } else {
                    mr0 = 0x1e14;
                    tcl = 7;
                    wr_latency = 3;
                    tcwl = 5;
                    mr2 = 32;
                }

                twtp = tcwl + 2 + twtr; // WL+BL/2+tWTR
                trd2wr = tcwl + 2 + twr; // WL+BL/2+tWR
                twr2rd = tcwl + twtr; // WL+tWTR

                tdinit0 = 500 * freq + 1; // 500 us
                tdinit1 = 360 * freq / 1000 + 1; // 360 ns
                tdinit2 = 200 * freq + 1; // 200 us
                tdinit3 = 1 * freq + 1; //   1 us

                if ((tpr13 >> 2) & 0x03) == 0x01 || freq < 912 {
                    mr1 = dmr1;
                    t_rdata_en = tcwl; // a5 <- a4
                    tcksrx = 5;
                    tckesr = 4;
                    trd2wr = 5;
                } else {
                    mr1 = dmr1;
                    t_rdata_en = tcwl; // a5 <- a4
                    tcksrx = 5;
                    tckesr = 4;
                    trd2wr = 6;
                }
                tcke = 3; // not in .S ?
                tmod = 12;
                tmrd = 4;
                tmrw = 0;
                mr3 = 0;
            }
            // #if 1
            6 =>
            // LPDDR2
            {
                trasmax = freq / 60;
                mr3 = dmr3;
                twtp = twr + 5;
                mr2 = 6;
                mr1 = 5;
                tcksrx = 5;
                tckesr = 5;
                trd2wr = 10;
                tcke = 2;
                tmod = 5;
                tmrd = 5;
                tmrw = 3;
                tcl = 4;
                wr_latency = 1;
                t_rdata_en = 1;
                tdinit0 = 200 * freq + 1;
                tdinit1 = 100 * freq / 1000 + 1;
                tdinit2 = 11 * freq + 1;
                tdinit3 = 1 * freq + 1;
                twr2rd = twtr + 5;
                tcwl = 2;
                mr1 = 195;
                mr0 = 0;
            }

            7 =>
            // LPDDR3
            {
                trasmax = freq / 60;
                if freq < 800 {
                    tcwl = 4;
                    wr_latency = 3;
                    t_rdata_en = 6;
                    mr2 = 12;
                } else {
                    tcwl = 3;
                    tcke = 6;
                    wr_latency = 2;
                    t_rdata_en = 5;
                    mr2 = 10;
                }
                twtp = tcwl + 5;
                tcl = 7;
                mr3 = dmr3;
                tcksrx = 5;
                tckesr = 5;
                trd2wr = 13;
                tcke = 3;
                tmod = 12;
                tdinit0 = 400 * freq + 1;
                tdinit1 = 500 * freq / 1000 + 1;
                tdinit2 = 11 * freq + 1;
                tdinit3 = 1 * freq + 1;
                tmrd = 5;
                tmrw = 5;
                twr2rd = tcwl + twtr + 5;
                mr1 = 195;
                mr0 = 0;
            }
            // #endif
            _ => {
                twr2rd = 8; // 48(sp)
                tcksrx = 4; // t1
                tckesr = 3; // t4
                trd2wr = 4; // t6
                trasmax = 27; // t3
                twtp = 12; // s6
                tcke = 2; // s8
                tmod = 6; // t0
                tmrd = 2; // t5
                tmrw = 0; // a1
                tcwl = 3; // a5
                tcl = 3; // a0
                wr_latency = 1; // a7
                t_rdata_en = 1; // a4
                mr3 = 0; // s0
                mr2 = 0; // t2
                mr1 = 0; // s1
                mr0 = 0; // a3
                tdinit3 = 0; // 40(sp)
                tdinit2 = 0; // 32(sp)
                tdinit1 = 0; // 24(sp)
                tdinit0 = 0; // 16(sp)
            }
        }
        if trtp < tcl - trp + 2 {
            trtp = tcl - trp + 2;
        }
        trtp = 4;

        // Update mode block when permitted
        if (self.dram_mr0 & 0xffff0000) == 0 {
            self.dram_mr0 = mr0;
        }
        if (self.dram_mr1 & 0xffff0000) == 0 {
            self.dram_mr1 = mr1;
        }
        if (self.dram_mr2 & 0xffff0000) == 0 {
            self.dram_mr2 = mr2;
        }
        if (self.dram_mr3 & 0xffff0000) == 0 {
            self.dram_mr3 = mr3;
        }

        // Set mode registers
        write32(0x3103030, self.dram_mr0);
        write32(0x3103034, self.dram_mr1);
        write32(0x3103038, self.dram_mr2);
        write32(0x310303c, self.dram_mr3);
        write32(0x310302c, (self.dram_odt_en >> 4) & 0x3); // ??

        // Set dram timing DRAMTMG0 - DRAMTMG5
        reg_val =
            ((twtp as u32) << 24) | ((tfaw as u32) << 16) | (trasmax << 8) | ((tras as u32) << 0);
        write32(0x3103058, reg_val);
        reg_val = ((txp as u32) << 16) | ((trtp as u32) << 8) | ((trc as u32) << 0);
        write32(0x310305c, reg_val);
        reg_val = ((tcwl as u32) << 24)
            | ((tcl as u32) << 16)
            | ((trd2wr as u32) << 8)
            | ((twr2rd as u32) << 0);
        write32(0x3103060, reg_val);
        reg_val = (tmrw << 16) | (tmrd << 12) | (tmod << 0);
        write32(0x3103064, reg_val);
        reg_val = ((trcd as u32) << 24)
            | ((tccd as u32) << 16)
            | ((trrd as u32) << 8)
            | ((trp as u32) << 0);
        write32(0x3103068, reg_val);
        reg_val = (tcksrx << 24) | (tcksrx << 16) | (tckesr << 8) | (tcke << 0);
        write32(0x310306c, reg_val);

        // Set two rank timing
        reg_val = read32(0x3103078);
        reg_val &= 0x0fff0000;
        reg_val |= if self.dram_clk < 800 {
            0xf0006600
        } else {
            0xf0007600
        };
        reg_val |= 0x10;
        write32(0x3103078, reg_val);

        // Set phy interface time PITMG0, PTR3, PTR4
        reg_val = ((0x2 as u32) << 24)
            | ((t_rdata_en as u32) << 16)
            | ((0x1 as u32) << 8)
            | ((wr_latency as u32) << 0);
        write32(0x3103080, reg_val);
        write32(0x3103050, (tdinit0 << 0) | (tdinit1 << 20));
        write32(0x3103054, (tdinit2 << 0) | (tdinit3 << 20));

        // Set refresh timing and mode
        reg_val = ((trefi as u32) << 16) | ((trfc as u32) << 0);
        write32(0x3103090, reg_val);
        reg_val = 0x0fff0000 & ((trefi as u32) << 15);
        write32(0x3103094, reg_val);
    }

    // This routine seems to have several remapping tables for 22 lines.
    // It is unclear which lines are being remapped. It seems to pick
    // table cfg7 for the Nezha board.
    //
    fn mctl_phy_ac_remapping(mut self) {
        //        cfg0[] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
        // static cfg1[] = {1, 9, 3, 7, 8, 18, 4, 13, 5, 6, 10, 2, 14, 12, 0, 0, 21, 17, 20, 19, 11, 22};
        // static cfg2[] = {4, 9, 3, 7, 8, 18, 1, 13, 2, 6, 10, 5, 14, 12, 0, 0, 21, 17, 20, 19, 11, 22};
        // static cfg3[] = {1, 7, 8, 12, 10, 18, 4, 13, 5, 6, 3, 2, 9, 0, 0, 0, 21, 17, 20, 19, 11, 22};
        // static cfg4[] = {4, 12, 10, 7, 8, 18, 1, 13, 2, 6, 3, 5, 9, 0, 0, 0, 21, 17, 20, 19, 11, 22};
        // static cfg5[] = {13, 2, 7, 9, 12, 19, 5, 1, 6, 3, 4, 8, 10, 0, 0, 0, 21, 22, 18, 17, 11, 20};
        // static cfg6[] = {3, 10, 7, 13, 9, 11, 1, 2, 4, 6, 8, 5, 12, 0, 0, 0, 20, 1, 0, 21, 22, 17};
        // static cfg7[] = {3, 2, 4, 7, 9, 1, 17, 12, 18, 14, 13, 8, 15, 6, 10, 5, 19, 22, 16, 21, 20, 11};

        let mut fuse: u32 = 0;
        let mut val: u32 = 0;

        fuse = (read32(0x3006228) << 0x14) >> 0x1c;
        print!("ddr_efuse_type: 0x{:x?}\r\n", fuse);

        val = (read32(0x03006200) << 0x10) >> 0x18;
        print!("mark_id: 0x{:x?}\r\n", val);

        if ((self.dram_tpr13 >> 18) & 0x3) != 0 {
            // memcpy_self(cfg0, cfg7, 22);
        } else {
            match fuse {
                // #if 1
                8 => {
                    // memcpy_self(cfg0, cfg2, 22);
                }
                9 => {
                    // memcpy_self(cfg0, cfg3, 22);
                }
                // #endif
                10 => {
                    // memcpy_self(cfg0, cfg5, 22);
                }
                // #if 1
                11 => {
                    // memcpy_self(cfg0, cfg4, 22);
                }

                12 => {
                    // memcpy_self(cfg0, cfg1, 22);
                }
                13 => {}
                14 => {}
                _ => {} // #endif
            }
        }

        if self.dram_type == 2 {
            if fuse == 15 {
                return;
            }

            // memcpy_self(cfg0, cfg6, 22);
        }

        // if (self.dram_type == 2 || self.dram_type == 3) {
        // 	val = (cfg0[4] << 25) | (cfg0[3] << 20) | (cfg0[2] << 15) | (cfg0[1] << 10) | (cfg0[0] << 5);
        // 	write32(0x3102500, val);

        // 	val = (cfg0[10] << 25) | (cfg0[9] << 20) | (cfg0[8] << 15) | (cfg0[7] << 10) | (cfg0[6] << 5) | cfg0[5];
        // 	write32(0x3102504, val);

        // 	val = (cfg0[15] << 20) | (cfg0[14] << 15) | (cfg0[13] << 10) | (cfg0[12] << 5) | cfg0[11];
        // 	write32(0x3102508, val);

        // 	val = (cfg0[21] << 25) | (cfg0[20] << 20) | (cfg0[19] << 15) | (cfg0[18] << 10) | (cfg0[17] << 5) | cfg0[16];
        // 	write32(0x310250c, val);

        // 	val = (cfg0[4] << 25) | (cfg0[3] << 20) | (cfg0[2] << 15) | (cfg0[1] << 10) | (cfg0[0] << 5) | 1;
        // 	write32(0x3102500, val);
        // }
    }

    // The main purpose of this routine seems to be to copy an address configuration
    // from the dram_para1 and dram_para2 fields to the PHY configuration registers
    // (0x3102000, 0x3102004).
    //
    fn mctl_com_init(mut self) {
        let mut val: u32 = 0;
        let mut end: u32 = 0;
        let mut ptr: u32 = 0;
        let mut i: i32 = 0;

        // purpose ??
        val = read32(0x3102008) & 0xffffc0ff;
        val |= 0x2000;
        write32(0x3102008, val);

        // Set sdram type and word width
        val = read32(0x3102000) & 0xff000fff;
        val |= (self.dram_type & 0x7) << 16; // DRAM type
        val |= (!self.dram_para2 & 0x1) << 12; // DQ width
        if (self.dram_type) != 6 && (self.dram_type) != 7 {
            val |= ((self.dram_tpr13 >> 5) & 0x1) << 19; // 2T or 1T
            val |= 0x400000;
        } else {
            val |= 0x480000; // type 6 and 7 must use 1T
        }
        write32(0x3102000, val);

        // init rank / bank / row for single/dual or two different ranks
        val = self.dram_para2;
        end = if (val & 0x100 != 0) && (((val >> 12) & 0xf) != 1) {
            32
        } else {
            16
        };
        ptr = 0x3102000;

        i = 0;
        while i != end as i32 {
            val = read32(ptr) & 0xfffff000;

            val |= (self.dram_para2 >> 12) & 0x3; // rank
            val |= ((self.dram_para1 >> (i + 12)) << 2) & 0x4; // bank - 2
            val |= (((self.dram_para1 >> (i + 4)) - 1) << 4) & 0xff; // row - 1

            // convert from page size to column addr width - 3
            match (self.dram_para1 >> i) & 0xf {
                8 => {
                    val |= 0xa00;
                }
                4 => {
                    val |= 0x900;
                }
                2 => {
                    val |= 0x800;
                }
                1 => {
                    val |= 0x700;
                }
                _ => {
                    val |= 0x600;
                }
            }

            write32(ptr, val);
            ptr += 4;
            i += 16;
        }

        // set ODTMAP based on number of ranks in use
        val = if (read32(0x3102000) & 0x1) != 0 {
            0x303
        } else {
            0x201
        };
        write32(0x3103120, val);

        // set mctl reg 3c4 to zero when using half DQ
        if (self.dram_para2 & (1 << 0)) != 0 {
            write32(0x31033c4, 0);
        }

        // purpose ??
        if (self.dram_tpr4) != 0 {
            val = read32(0x3102000);
            val |= (self.dram_tpr4 << 25) & 0x06000000;
            write32(0x3102000, val);

            val = read32(0x3102004);
            val |= ((self.dram_tpr4 >> 2) << 12) & 0x001ff000;
            write32(0x3102004, val);
        }
    }

    // Set the Vref mode for the controller
    //
    fn mctl_vrefzq_init(self) {
        let mut val: u32 = 0;

        if (self.dram_tpr13 & (1 << 17)) == 0 {
            val = read32(0x3103110) & 0x80808080; // IOCVR0
            val |= self.dram_tpr5;
            write32(0x3103110, val);

            if (self.dram_tpr13 & (1 << 16)) == 0 {
                val = read32(0x3103114) & 0xffffff80; // IOCVR1
                val |= self.dram_tpr6 & 0x7f;
                write32(0x3103114, val);
            }
        }
    }
    // Main purpose of sys_init seems to be to initalise the clocks for
    // the sdram controller.
    //
    fn mctl_sys_init(mut self) {
        let mut val: u32 = 0;

        // s1 = 0x02001000

        // assert MBUS reset
        write32(0x2001540, read32(0x2001540) & 0xbfffffff);

        // turn off sdram clock gate, assert sdram reset
        write32(0x200180c, read32(0x200180c) & 0xfffefffe);
        write32(0x2001800, (read32(0x2001800) & 0x3fffffff) | 0x8000000);
        sdelay(10);

        // set ddr pll clock
        val = self.ccm_set_pll_ddr_clk(0);
        self.dram_clk = val >> 1;
        sdelay(100);
        self.dram_disable_all_master();

        // release sdram reset
        write32(0x200180c, read32(0x200180c) | 0x10000);

        // release MBUS reset
        write32(0x2001540, read32(0x2001540) | 0x40000000);
        write32(0x2001800, read32(0x2001800) | 0x40000000);

        sdelay(5);

        // turn on sdram clock gate
        write32(0x200180c, read32(0x200180c) | 0x1);

        // turn dram clock gate on, trigger sdr clock update
        write32(0x2001800, read32(0x2001800) | 0x88000000);
        sdelay(5);

        // mCTL clock enable
        write32(0x310300c, 0x8000);
        sdelay(10);
    }

    // Purpose of this routine seems to be to initialize the PLL driving
    // the MBUS and sdram.
    //
    fn ccm_set_pll_ddr_clk(self, index: i32) -> u32 {
        let mut val: u32 = 0;
        let mut clk: u32 = 0;
        let mut n: u32 = 0;

        clk = if (self.dram_tpr13 & (1 << 6)) != 0 {
            self.dram_tpr9
        } else {
            self.dram_clk
        };

        // set VCO clock divider
        n = (clk * 2) / 24;

        val = read32(0x2001010);
        val &= 0xfff800fc; // clear dividers
        val |= (n - 1) << 8; // set PLL division
        val |= 0xc0000000; // enable PLL and LDO
        val &= 0xdfffffff;
        write32(0x2001010, val | 0x20000000);

        // wait for PLL to lock
        while (read32(0x2001010) & 0x10000000) == 0 {}

        sdelay(20);

        // enable PLL output
        val = read32(0x2001000);
        val |= 0x08000000;
        write32(0x2001000, val);

        // turn clock gate on
        val = read32(0x2001800);
        val &= 0xfcfffcfc; // select DDR clk source, n=1, m=1
        val |= 0x80000000; // turn clock on
        write32(0x2001800, val);

        return n * 24;
        
    }

    fn get_size(self) -> u32 {
        return 0u32;
    }

    fn enable_all_master(self) {
        write32(0x3102020, u32::MAX); //-1
        write32(0x3102024, 0xff);
        write32(0x3102028, 0xffff);
        sdelay(10);
    }

    fn dram_disable_all_master(self) {
        write32(0x3102020, 1);
        write32(0x3102024, 0);
        write32(0x3102028, 0);
        sdelay(10);
    }

    fn dramc_simple_wr_test(mem_size: u32, test: u32) -> u32 {
        0u32
    }
 
}
