pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: u8,
    pub program_counter: u16,
    pub stack_pointer: u8,
    memory: [u8; 0xffff],
}

#[derive(Debug, PartialEq)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Relative,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
}

impl Default for CPU {
    fn default() -> Self {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            status: 0,
            program_counter: 0,
            stack_pointer: 0xFD,
            memory: [0; 0xffff],
        }
    }
}

impl CPU {
    pub fn new() -> Self {
        CPU::default()
    }

    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }

    fn mem_read_u16(&self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | lo
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }

    fn stack_pop(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.mem_read(0x0100 + self.stack_pointer as u16)
    }

    fn stack_push(&mut self, data: u8) {
        self.mem_write(0x0100 + self.stack_pointer as u16, data);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    fn stack_pop_u16(&mut self) -> u16 {
        let lo = self.stack_pop() as u16;
        let hi = self.stack_pop() as u16;
        (hi << 8) | lo
    }

    fn stack_push_u16(&mut self, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.stack_push(hi);
        self.stack_push(lo);
    }

    fn get_operand_address(&self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Implied => {
                panic!("AddressingMode::Implied");
            }

            AddressingMode::Accumulator => {
                panic!("AddressingMode::Accumulator");
            }

            AddressingMode::Immediate => self.program_counter,

            AddressingMode::ZeroPage => self.mem_read(self.program_counter) as u16,

            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.program_counter);
                pos.wrapping_add(self.register_x) as u16
            }

            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.program_counter);
                pos.wrapping_add(self.register_y) as u16
            }

            AddressingMode::Relative => {
                let base = self.mem_read(self.program_counter) as i8;
                (base as u16).wrapping_add(self.program_counter + 1)
            }

            AddressingMode::Absolute => self.mem_read_u16(self.program_counter),

            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(self.program_counter);
                base.wrapping_add(self.register_x as u16)
            }

            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(self.program_counter);
                base.wrapping_add(self.register_y as u16)
            }

            AddressingMode::Indirect => {
                let base = self.mem_read_u16(self.program_counter);
                self.mem_read_u16(base)
            }

            AddressingMode::Indirect_X => {
                let base = self.mem_read(self.program_counter);
                let ptr = base.wrapping_add(self.register_x);
                self.mem_read_u16(ptr as u16)
            }

            AddressingMode::Indirect_Y => {
                let base = self.mem_read(self.program_counter);
                let deref_base = self.mem_read_u16(base as u16);
                deref_base.wrapping_add(self.register_y as u16)
            }

            AddressingMode::NoneAddressing => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
        if result == 0 {
            self.status |= 0b00000010;
        } else {
            self.status &= !0b00000010;
        }

        if result & 0b10000000 != 0 {
            self.status |= 0b10000000;
        } else {
            self.status &= !0b10000000;
        }
    }

    fn set_register_a(&mut self, value: u8) {
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn add_to_register_a(&mut self, value: u8) {
        let carry = self.status & 0b00000001;
        let (result, carry_flag) = self.register_a.overflowing_add(value + carry);
        let overflow_flag = (self.register_a & 0b10000000) == (value & 0b10000000)
            && (value & 0b10000000) != (result & 0b10000000);

        if carry_flag {
            self.status |= 0b00000001
        } else {
            self.status &= !0b00000001
        };

        if overflow_flag {
            self.status |= 0b01000000
        } else {
            self.status &= !0b01000000
        };

        self.set_register_a(result);
    }

    fn branch(&mut self, condition: bool) {
        if condition {
            let addr = self.get_operand_address(&AddressingMode::Relative);
            self.program_counter = addr;
        }
    }

    fn compare(&mut self, mode: &AddressingMode, target: u8) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let result = target.wrapping_sub(value);
        let carry_flag = target >= value;

        if carry_flag {
            self.status |= 0b00000001
        } else {
            self.status &= !0b00000001
        };

        self.update_zero_and_negative_flags(result);
    }

    fn adc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.add_to_register_a(value);
    }

    fn and(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_a(self.register_a & value);
    }

    fn asl(&mut self, mode: &AddressingMode) {
        let (result, carry_flag) = if mode == &AddressingMode::Accumulator {
            let (result, carry_flag) = self.register_a.overflowing_mul(2);
            self.register_a = result;
            (result, carry_flag)
        } else {
            let addr = self.get_operand_address(mode);
            let value = self.mem_read(addr);
            let (result, carry_flag) = value.overflowing_mul(2);
            self.mem_write(addr, result);
            (result, carry_flag)
        };

        if carry_flag {
            self.status |= 0b00000001
        } else {
            self.status &= !0b00000001
        };

        self.update_zero_and_negative_flags(result);
    }

    fn bcc(&mut self, _mode: &AddressingMode) {
        self.branch(self.status & 0b00000001 != 0b00000001);
    }

    fn bcs(&mut self, _mode: &AddressingMode) {
        self.branch(self.status & 0b00000001 == 0b00000001);
    }

    fn beq(&mut self, _mode: &AddressingMode) {
        self.branch(self.status & 0b00000010 == 0b00000010);
    }

    fn bit(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let result = self.register_a & value;

        if result == 0 {
            self.status |= 0b00000010;
        } else {
            self.status &= !0b00000010;
        }

        if value & 0b10000000 != 0 {
            self.status |= 0b10000000;
        } else {
            self.status &= !0b10000000;
        }

        if value & 0b01000000 != 0 {
            self.status |= 0b01000000;
        } else {
            self.status &= !0b01000000;
        }
    }

    fn bmi(&mut self, _mode: &AddressingMode) {
        self.branch(self.status & 0b10000000 == 0b10000000);
    }

    fn bne(&mut self, _mode: &AddressingMode) {
        self.branch(self.status & 0b00000010 != 0b00000010);
    }

    fn bpl(&mut self, _mode: &AddressingMode) {
        self.branch(self.status & 0b10000000 != 0b10000000);
    }

    fn bvc(&mut self, _mode: &AddressingMode) {
        self.branch(self.status & 0b01000000 != 0b01000000);
    }

    fn bvs(&mut self, _mode: &AddressingMode) {
        self.branch(self.status & 0b01000000 == 0b01000000);
    }

    fn clc(&mut self, _mode: &AddressingMode) {
        self.status &= !0b00000001;
    }

    fn cld(&mut self, _mode: &AddressingMode) {
        self.status &= !0b00001000;
    }

    fn cli(&mut self, _mode: &AddressingMode) {
        self.status &= !0b00000100;
    }

    fn clv(&mut self, _mode: &AddressingMode) {
        self.status &= !0b01000000;
    }

    fn cmp(&mut self, mode: &AddressingMode) {
        self.compare(mode, self.register_a);
    }

    fn cpx(&mut self, mode: &AddressingMode) {
        self.compare(mode, self.register_x);
    }

    fn cpy(&mut self, mode: &AddressingMode) {
        self.compare(mode, self.register_y);
    }

    fn dec(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let result = value.wrapping_sub(1);
        self.mem_write(addr, result);
        self.update_zero_and_negative_flags(result);
    }

    fn dex(&mut self, _mode: &AddressingMode) {
        self.register_x = self.register_x.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn dey(&mut self, _mode: &AddressingMode) {
        self.register_y = self.register_y.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn eor(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_a(self.register_a ^ value);
    }

    fn inc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let result = value.wrapping_add(1);
        self.mem_write(addr, result);
        self.update_zero_and_negative_flags(result);
    }

    fn inx(&mut self, _mode: &AddressingMode) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn iny(&mut self, _mode: &AddressingMode) {
        self.register_y = self.register_y.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn jmp(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.program_counter = addr;
    }

    fn jsr(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.stack_push_u16(self.program_counter + 2 - 1);
        self.program_counter = addr;
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_y = value;
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn lsr(&mut self, mode: &AddressingMode) {
        let (result, carry_flag) = if mode == &AddressingMode::Accumulator {
            let result = self.register_a / 2;
            let carry_flag = self.register_a & 0b00000001 == 0b00000001;
            self.register_a = result;
            (result, carry_flag)
        } else {
            let addr = self.get_operand_address(mode);
            let value = self.mem_read(addr);
            let result = value / 2;
            let carry_flag = value & 0b00000001 == 0b00000001;
            self.mem_write(addr, result);
            (result, carry_flag)
        };

        if carry_flag {
            self.status |= 0b00000001
        } else {
            self.status &= !0b00000001
        };

        self.update_zero_and_negative_flags(result);
    }

    fn nop(&mut self, _mode: &AddressingMode) {
        // do nothing
    }

    fn ora(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_a(self.register_a | value);
    }

    fn pha(&mut self, _mode: &AddressingMode) {
        self.stack_push(self.register_a);
    }

    fn php(&mut self, _mode: &AddressingMode) {
        self.stack_push(self.status | 0b00010000 | 0b00100000);
    }

    fn pla(&mut self, _mode: &AddressingMode) {
        let value = self.stack_pop();
        self.set_register_a(value);
    }

    fn plp(&mut self, _mode: &AddressingMode) {
        let value = self.stack_pop();
        self.status = value & !0b00010000 | 0b00100000;
    }

    fn rol(&mut self, mode: &AddressingMode) {
        let (result, carry_flag) = if mode == &AddressingMode::Accumulator {
            let (result, carry_flag) = self.register_a.overflowing_mul(2);
            let result = result | (self.status & 0b00000001);
            self.register_a = result;
            (result, carry_flag)
        } else {
            let addr = self.get_operand_address(mode);
            let value = self.mem_read(addr);
            let (result, carry_flag) = value.overflowing_mul(2);
            let result = result | (self.status & 0b00000001);
            self.mem_write(addr, result);
            (result, carry_flag)
        };

        if carry_flag {
            self.status |= 0b00000001
        } else {
            self.status &= !0b00000001
        };

        self.update_zero_and_negative_flags(result);
    }

    fn ror(&mut self, mode: &AddressingMode) {
        let (result, carry_flag) = if mode == &AddressingMode::Accumulator {
            let result = self.register_a / 2;
            let result = result | (self.status & 0b00000001) << 7;
            let carry_flag = self.register_a & 0b00000001 == 0b00000001;
            self.register_a = result;
            (result, carry_flag)
        } else {
            let addr = self.get_operand_address(mode);
            let value = self.mem_read(addr);
            let result = value / 2;
            let result = result | (self.status & 0b00000001) << 7;
            let carry_flag = value & 0b00000001 == 0b00000001;
            self.mem_write(addr, result);
            (result, carry_flag)
        };

        if carry_flag {
            self.status |= 0b00000001
        } else {
            self.status &= !0b00000001
        };

        self.update_zero_and_negative_flags(result);
    }

    fn rti(&mut self, _mode: &AddressingMode) {
        let value = self.stack_pop();
        self.status = value & !0b00010000 | 0b00100000;
        self.program_counter = self.stack_pop_u16();
    }

    fn rts(&mut self, _mode: &AddressingMode) {
        let addr = self.stack_pop_u16() + 1;
        self.program_counter = addr;
    }

    fn sbc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.add_to_register_a(value.wrapping_neg().wrapping_sub(1));
    }

    fn sec(&mut self, _mode: &AddressingMode) {
        self.status |= 0b00000001;
    }

    fn sed(&mut self, _mode: &AddressingMode) {
        self.status |= 0b00001000;
    }

    fn sei(&mut self, _mode: &AddressingMode) {
        self.status |= 0b00000100;
    }

    fn sta(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }

    fn stx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_x);
    }

    fn sty(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_y);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x8000..(0x8000 + program.len())].copy_from_slice(&program[..]);
        self.mem_write_u16(0xfffc, 0x8000);
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.status = 0;

        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    pub fn run(&mut self) {
        loop {
            let opcode = self.mem_read(self.program_counter);
            self.program_counter += 1;

            match opcode {
                /* ADC */
                0x69 => {
                    self.adc(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* AND */
                0x29 => {
                    self.and(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* ASL */
                0x0A => {
                    self.asl(&AddressingMode::Accumulator);
                }
                0x06 => {
                    self.asl(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* BCC */
                0x90 => {
                    self.bcc(&AddressingMode::Relative);
                    self.program_counter += 1;
                }

                /* BCS */
                0xB0 => {
                    self.bcs(&AddressingMode::Relative);
                    self.program_counter += 1;
                }

                /* BEQ */
                0xF0 => {
                    self.beq(&AddressingMode::Relative);
                    self.program_counter += 1;
                }

                /* BIT */
                0x24 => {
                    self.bit(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* BMI */
                0x30 => {
                    self.bmi(&AddressingMode::Relative);
                    self.program_counter += 1;
                }

                /* BNE */
                0xD0 => {
                    self.bne(&AddressingMode::Relative);
                    self.program_counter += 1;
                }

                /* BPL */
                0x10 => {
                    self.bpl(&AddressingMode::Relative);
                    self.program_counter += 1;
                }

                /* BRK */
                0x00 => return,

                /* BVC */
                0x50 => {
                    self.bvc(&AddressingMode::Relative);
                    self.program_counter += 1;
                }

                /* BVS */
                0x70 => {
                    self.bvs(&AddressingMode::Relative);
                    self.program_counter += 1;
                }

                /* CLC */
                0x18 => self.clc(&AddressingMode::Implied),

                /* CLD */
                0xD8 => self.cld(&AddressingMode::Implied),

                /* CLI */
                0x58 => self.cli(&AddressingMode::Implied),

                /* CLV */
                0xB8 => self.clv(&AddressingMode::Implied),

                /* CMP */
                0xC9 => {
                    self.cmp(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* CPX */
                0xE0 => {
                    self.cpx(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* CPY */
                0xC0 => {
                    self.cpy(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* DEC */
                0xC6 => {
                    self.dec(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* DEX */
                0xCA => {
                    self.dex(&AddressingMode::Implied);
                }

                /* DEY */
                0x88 => {
                    self.dey(&AddressingMode::Implied);
                }

                /* EOR */
                0x49 => {
                    self.eor(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* INC */
                0xE6 => {
                    self.inc(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* INX */
                0xE8 => self.inx(&AddressingMode::Implied),

                /* INY */
                0xC8 => self.iny(&AddressingMode::Implied),

                /* JMP */
                0x4C => {
                    self.jmp(&AddressingMode::Absolute);
                }
                0x6C => {
                    self.jmp(&AddressingMode::Indirect);
                }

                /* JSR */
                0x20 => {
                    self.jsr(&AddressingMode::Absolute);
                    self.program_counter += 2;
                }

                /* LDA */
                0xA9 => {
                    self.lda(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }
                0xA5 => {
                    self.lda(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }
                0xB5 => {
                    self.lda(&AddressingMode::ZeroPage_X);
                    self.program_counter += 1;
                }
                0xAD => {
                    self.lda(&AddressingMode::Absolute);
                    self.program_counter += 2;
                }
                0xBD => {
                    self.lda(&AddressingMode::Absolute_X);
                    self.program_counter += 2;
                }
                0xB9 => {
                    self.lda(&AddressingMode::Absolute_Y);
                    self.program_counter += 2;
                }
                0xA1 => {
                    self.lda(&AddressingMode::Indirect_X);
                    self.program_counter += 1;
                }
                0xB1 => {
                    self.lda(&AddressingMode::Indirect_Y);
                    self.program_counter += 1;
                }

                /* LDX */
                0xA2 => {
                    self.ldx(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* LDY */
                0xA0 => {
                    self.ldy(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* LSR */
                0x4A => {
                    self.lsr(&AddressingMode::Accumulator);
                }
                0x46 => {
                    self.lsr(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* NOP */
                0xEA => self.nop(&AddressingMode::Implied),

                /* ORA */
                0x09 => {
                    self.ora(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* PHA */
                0x48 => {
                    self.pha(&AddressingMode::Implied);
                }

                /* PHP */
                0x08 => {
                    self.php(&AddressingMode::Implied);
                }

                /* PLA */
                0x68 => {
                    self.pla(&AddressingMode::Implied);
                }

                /* PLP */
                0x28 => {
                    self.plp(&AddressingMode::Implied);
                }

                /* ROL */
                0x2A => {
                    self.rol(&AddressingMode::Accumulator);
                }
                0x26 => {
                    self.rol(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* ROR */
                0x6A => {
                    self.ror(&AddressingMode::Accumulator);
                }
                0x66 => {
                    self.ror(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* RTI */
                0x40 => {
                    self.rti(&AddressingMode::Implied);
                }

                /* RTS */
                0x60 => {
                    self.rts(&AddressingMode::Implied);
                }

                /* SBC */
                0xE9 => {
                    self.sbc(&AddressingMode::Immediate);
                    self.program_counter += 1;
                }

                /* SEC */
                0x38 => self.sec(&AddressingMode::Implied),

                /* SED */
                0xF8 => self.sed(&AddressingMode::Implied),

                /* SEI */
                0x78 => self.sei(&AddressingMode::Implied),

                /* STA */
                0x85 => {
                    self.sta(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }
                0x95 => {
                    self.sta(&AddressingMode::ZeroPage_X);
                    self.program_counter += 1;
                }
                0x8D => {
                    self.sta(&AddressingMode::Absolute);
                    self.program_counter += 2;
                }
                0x9D => {
                    self.sta(&AddressingMode::Absolute_X);
                    self.program_counter += 2;
                }
                0x99 => {
                    self.sta(&AddressingMode::Absolute_Y);
                    self.program_counter += 2;
                }
                0x81 => {
                    self.sta(&AddressingMode::Indirect_X);
                    self.program_counter += 1;
                }
                0x91 => {
                    self.sta(&AddressingMode::Indirect_Y);
                    self.program_counter += 1;
                }

                /* STX */
                0x86 => {
                    self.stx(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* STY */
                0x84 => {
                    self.sty(&AddressingMode::ZeroPage);
                    self.program_counter += 1;
                }

                /* TAX */
                0xAA => self.tax(),

                _ => todo!(""),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /* AND */
    #[test]
    fn test_and() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x29, 0x0C, 0x00]);
        cpu.reset();
        cpu.register_a = 0x0A;
        cpu.run();
        assert_eq!(cpu.register_a, 0x08);
        assert_eq!(cpu.status, 0);
    }

    /* EOR */
    #[test]
    fn test_eor() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x49, 0x0C, 0x00]);
        cpu.reset();
        cpu.register_a = 0x0A;
        cpu.run();
        assert_eq!(cpu.register_a, 0x06);
        assert_eq!(cpu.status, 0);
    }

    /* INX */
    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xe8, 0xe8, 0x00]);
        cpu.reset();
        cpu.register_x = 0xff;
        cpu.run();
        assert_eq!(cpu.register_x, 1);
    }

    /* LDA */
    #[test]
    fn test_lda_immediate() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 0x05);
        assert_eq!(cpu.status, 0);
    }

    #[test]
    fn test_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x00, 0x00]);
        assert_eq!(cpu.status, 0b00000010);
    }

    #[test]
    fn test_lda_negative_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x80, 0x00]);
        assert_eq!(cpu.status, 0b10000000);
    }

    #[test]
    fn test_lda_zero_page() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xa5, 0x10, 0x00]);
        cpu.reset();
        cpu.mem_write(0x10, 0x55);
        cpu.run();
        assert_eq!(cpu.register_a, 0x55);
    }

    #[test]
    fn test_lda_zero_page_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xb5, 0x10, 0x00]);
        cpu.reset();
        cpu.mem_write(0x11, 0x56);
        cpu.register_x = 0x01;
        cpu.run();
        assert_eq!(cpu.register_a, 0x56);
    }

    #[test]
    fn test_lda_absolute() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xad, 0x10, 0x32, 0x00]);
        cpu.reset();
        cpu.mem_write(0x3210, 0x57);
        cpu.run();
        assert_eq!(cpu.register_a, 0x57);
    }

    #[test]
    fn test_lda_absolute_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xbd, 0x10, 0x32, 0x00]);
        cpu.reset();
        cpu.mem_write(0x3211, 0x58);
        cpu.register_x = 0x01;
        cpu.run();
        assert_eq!(cpu.register_a, 0x58);
    }

    #[test]
    fn test_lda_absolute_y() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xb9, 0x10, 0x32, 0x00]);
        cpu.reset();
        cpu.mem_write(0x3220, 0x59);
        cpu.register_y = 0x10;
        cpu.run();
        assert_eq!(cpu.register_a, 0x59);
    }

    #[test]
    fn test_lda_indirect_x() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xa1, 0x10, 0x00]);
        cpu.reset();
        cpu.mem_write_u16(0x11, 0x5432);
        cpu.mem_write(0x5432, 0x5a);
        cpu.register_x = 0x01;
        cpu.run();
        assert_eq!(cpu.register_a, 0x5a);
    }

    #[test]
    fn test_lda_indirect_y() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xb1, 0x10, 0x00]);
        cpu.reset();
        cpu.mem_write_u16(0x10, 0x7654);
        cpu.mem_write(0x7664, 0x5b);
        cpu.register_y = 0x10;
        cpu.run();
        assert_eq!(cpu.register_a, 0x5b);
    }

    /* ORA */
    #[test]
    fn test_ora() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x09, 0x0C, 0x00]);
        cpu.reset();
        cpu.register_a = 0x0A;
        cpu.run();
        assert_eq!(cpu.register_a, 0x0E);
        assert_eq!(cpu.status, 0);
    }

    /* STA */
    #[test]
    fn test_sta() {
        let mut cpu = CPU::new();
        cpu.load(vec![0x85, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a = 0x5c;
        cpu.run();
        assert_eq!(cpu.mem_read(0x10), 0x5c);
    }

    /* TAX */
    #[test]
    fn test_tax() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xaa, 0x00]);
        cpu.reset();
        cpu.register_a = 10;
        cpu.run();
        assert_eq!(cpu.register_x, 10);
    }

    /* Other */
    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);
        assert_eq!(cpu.register_x, 0xc1);
    }
}
