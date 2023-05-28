use crate::opcodes;
use std::collections::HashMap;

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

pub trait Mem {
    fn mem_read(&self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&self, pos: u16) -> u16;

    fn mem_write_u16(&mut self, pos: u16, data: u16);
}

impl Mem for CPU {
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
}

impl CPU {
    pub fn new() -> Self {
        CPU::default()
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

    fn tay(&mut self) {
        self.register_y = self.register_a;
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn tsx(&mut self) {
        self.register_x = self.stack_pointer;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn txa(&mut self) {
        self.register_a = self.register_x;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn txs(&mut self) {
        self.stack_pointer = self.register_x;
    }

    fn tya(&mut self) {
        self.register_a = self.register_y;
        self.update_zero_and_negative_flags(self.register_a);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x0600..(0x0600 + program.len())].copy_from_slice(&program[..]);
        self.mem_write_u16(0xfffc, 0x0600);
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.status = 0;

        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    pub fn run(&mut self) {
        self.run_with_callback(|_| {});
    }

    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut CPU),
    {
        let ref opcodes: HashMap<u8, &'static opcodes::OpCode> = *opcodes::OPCODES_MAP;

        loop {
            let code = self.mem_read(self.program_counter);
            self.program_counter += 1;
            let program_counter_state = self.program_counter;

            let opcode = opcodes.get(&code).unwrap();

            match code {
                /* ADC */
                0x69 | 0x65 | 0x75 | 0x6d | 0x7d | 0x79 | 0x61 | 0x71 => self.adc(&opcode.mode),

                /* AND */
                0x29 | 0x25 | 0x35 | 0x2d | 0x3d | 0x39 | 0x21 | 0x31 => self.and(&opcode.mode),

                /* ASL */
                0x0a | 0x06 | 0x16 | 0x0e | 0x1e => self.asl(&opcode.mode),

                /* BCC */
                0x90 => self.bcc(&opcode.mode),

                /* BCS */
                0xb0 => self.bcs(&opcode.mode),

                /* BEQ */
                0xf0 => self.beq(&opcode.mode),

                /* BIT */
                0x24 | 0x2c => self.bit(&opcode.mode),

                /* BMI */
                0x30 => self.bmi(&opcode.mode),

                /* BNE */
                0xd0 => self.bne(&opcode.mode),

                /* BPL */
                0x10 => self.bpl(&opcode.mode),

                /* BRK */
                0x00 => return,

                /* BVC */
                0x50 => self.bvc(&opcode.mode),

                /* BVS */
                0x70 => self.bvs(&opcode.mode),

                /* CLC */
                0x18 => self.clc(&opcode.mode),

                /* CLD */
                0xd8 => self.cld(&opcode.mode),

                /* CLI */
                0x58 => self.cli(&opcode.mode),

                /* CLV */
                0xb8 => self.clv(&opcode.mode),

                /* CMP */
                0xc9 | 0xc5 | 0xd5 | 0xcd | 0xdd | 0xd9 | 0xc1 | 0xd1 => self.cmp(&opcode.mode),

                /* CPX */
                0xe0 | 0xe4 | 0xec => self.cpx(&opcode.mode),

                /* CPY */
                0xc0 | 0xc4 | 0xcc => self.cpy(&opcode.mode),

                /* DEC */
                0xc6 | 0xd6 | 0xce | 0xde => self.dec(&opcode.mode),

                /* DEX */
                0xca => self.dex(&opcode.mode),

                /* DEY */
                0x88 => self.dey(&opcode.mode),

                /* EOR */
                0x49 | 0x45 | 0x55 | 0x4d | 0x5d | 0x59 | 0x41 | 0x51 => self.eor(&opcode.mode),

                /* INC */
                0xe6 | 0xf6 | 0xee | 0xfe => self.inc(&opcode.mode),

                /* INX */
                0xe8 => self.inx(&opcode.mode),

                /* INY */
                0xc8 => self.iny(&opcode.mode),

                /* JMP */
                0x4c | 0x6c => self.jmp(&opcode.mode),

                /* JSR */
                0x20 => self.jsr(&opcode.mode),

                /* LDA */
                0xa9 | 0xa5 | 0xb5 | 0xad | 0xbd | 0xb9 | 0xa1 | 0xb1 => self.lda(&opcode.mode),

                /* LDX */
                0xa2 | 0xa6 | 0xb6 | 0xae | 0xbe => self.ldx(&opcode.mode),

                /* LDY */
                0xa0 | 0xa4 | 0xb4 | 0xac | 0xbc => self.ldy(&opcode.mode),

                /* LSR */
                0x4a | 0x46 | 0x56 | 0x4e | 0x5e => self.lsr(&opcode.mode),

                /* NOP */
                0xea => self.nop(&opcode.mode),

                /* ORA */
                0x09 | 0x05 | 0x15 | 0x0d | 0x1d | 0x19 | 0x01 | 0x11 => self.ora(&opcode.mode),

                /* PHA */
                0x48 => self.pha(&opcode.mode),

                /* PHP */
                0x08 => self.php(&opcode.mode),

                /* PLA */
                0x68 => self.pla(&opcode.mode),

                /* PLP */
                0x28 => self.plp(&opcode.mode),

                /* ROL */
                0x2a | 0x26 | 0x36 | 0x2e | 0x3e => self.rol(&opcode.mode),

                /* ROR */
                0x6a | 0x66 | 0x76 | 0x6e | 0x7e => self.ror(&opcode.mode),

                /* RTI */
                0x40 => self.rti(&opcode.mode),

                /* RTS */
                0x60 => self.rts(&opcode.mode),

                /* SBC */
                0xe9 | 0xe5 | 0xf5 | 0xed | 0xfd | 0xf9 | 0xe1 | 0xf1 => self.sbc(&opcode.mode),

                /* SEC */
                0x38 => self.sec(&opcode.mode),

                /* SED */
                0xf8 => self.sed(&opcode.mode),

                /* SEI */
                0x78 => self.sei(&opcode.mode),

                /* STA */
                0x85 | 0x95 | 0x8d | 0x9d | 0x99 | 0x81 | 0x91 => self.sta(&opcode.mode),

                /* STX */
                0x86 | 0x96 | 0x8e => self.stx(&opcode.mode),

                /* STY */
                0x84 | 0x94 | 0x8c => self.sty(&opcode.mode),

                /* TAX */
                0xaa => self.tax(),

                /* TAY */
                0xa8 => self.tay(),

                /* TSX */
                0xba => self.tsx(),

                /* TXA */
                0x8a => self.txa(),

                /* TXS */
                0x9a => self.txs(),

                /* TYA */
                0x98 => self.tya(),

                _ => todo!(""),
            }

            if program_counter_state == self.program_counter {
                self.program_counter += (opcode.len - 1) as u16;
            }

            callback(self);
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
