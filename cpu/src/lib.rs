// cpu/src/lib.rs
use memory::Bus;

/// Banderas del registro F (Flags)
const Z_FLAG: u8 = 0x80; // Zero: Resultado fue 0
const N_FLAG: u8 = 0x40; // Subtraction: La última operación fue resta
const H_FLAG: u8 = 0x20; // Half Carry: Acarreo del bit 3 al 4 (para BCD/DAA)
const C_FLAG: u8 = 0x10; // Carry: Acarreo del bit 7 (Overflow)

pub struct Registers {
    pub a: u8, pub f: u8,
    pub b: u8, pub c: u8,
    pub d: u8, pub e: u8,
    pub h: u8, pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Registers {
    pub fn new() -> Self {
        // Valores iniciales (bootrom bypass)
        Self {
            a: 0x01, f: 0xB0,
            b: 0x00, c: 0x13,
            d: 0x00, e: 0xD8,
            h: 0x01, l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }

    // Helpers para pares de 16 bits
    pub fn get_bc(&self) -> u16 { (self.b as u16) << 8 | (self.c as u16) }
    pub fn set_bc(&mut self, val: u16) { self.b = (val >> 8) as u8; self.c = val as u8; }

    pub fn get_de(&self) -> u16 { (self.d as u16) << 8 | (self.e as u16) }
    pub fn set_de(&mut self, val: u16) { self.d = (val >> 8) as u8; self.e = val as u8; }

    pub fn get_hl(&self) -> u16 { (self.h as u16) << 8 | (self.l as u16) }
    pub fn set_hl(&mut self, val: u16) { self.h = (val >> 8) as u8; self.l = val as u8; }

    pub fn get_af(&self) -> u16 { (self.a as u16) << 8 | (self.f as u16) }
    pub fn set_af(&mut self, val: u16) { self.a = (val >> 8) as u8; self.f = (val as u8) & 0xF0; }
}

pub struct Cpu {
    pub regs: Registers,
    pub ime: bool,    // Interrupt Master Enable
    pub halted: bool, // Estado de bajo consumo
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            ime: false,
            halted: false,
        }
    }

    /// Ciclo principal: Fetch, Decode, Execute
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        // Manejo básico de HALT e Interrupciones
        if self.halted {
            if self.ime && (bus.interrupt_flag & bus.interrupt_enable & 0x1F) != 0 {
                self.halted = false;
            } else {
                return 1; // Ciclo de espera
            }
        }
        

        // 2. Manejo de INTERRUPCIONES REAL
        if self.ime {
            // Verificamos qué interrupciones están activas Y habilitadas
            let fired = bus.interrupt_flag & bus.interrupt_enable;
            
            if (fired & 0x1F) != 0 {
                self.ime = false; // Deshabilitar interrupciones anidadas
                
                // Si estaba en HALT, despertamos
                self.halted = false;

                // PUSH PC al Stack (guardamos dónde estábamos)
                self.push(bus, self.regs.pc);

                // Identificar cuál saltó y mover el PC al Vector correspondiente
                // Prioridad: VBlank > LCD > Timer > Serial > Joypad
                if (fired & 0x01) != 0 {
                    self.regs.pc = 0x0040;       // Vector V-Blank
                    bus.interrupt_flag &= !0x01; // Limpiar flag
                } else if (fired & 0x02) != 0 {
                    self.regs.pc = 0x0048;       // Vector LCD Stat
                    bus.interrupt_flag &= !0x02;
                } else if (fired & 0x04) != 0 {
                    self.regs.pc = 0x0050;       // Vector Timer
                    bus.interrupt_flag &= !0x04;
                } else if (fired & 0x08) != 0 {
                    self.regs.pc = 0x0058;       // Vector Serial
                    bus.interrupt_flag &= !0x08;
                } else if (fired & 0x10) != 0 {
                    self.regs.pc = 0x0060;       // Vector Joypad
                    bus.interrupt_flag &= !0x10;
                }
                
                return 5; // Atender la interrupción consume 5 M-Cycles (20 T-Cycles)
            }
        }

        let opcode = self.fetch(bus);

        match opcode {
            // --- GRUPO 1: Cargas de 8 bits y Control ---
            0x00 => { 1 } // NOP
            0x10 => { // STOP
                self.fetch(bus); // Consume el byte siguiente (usualmente 0)
                // En un emu simple, STOP es casi como HALT o NOP
                1
            }
            0x06 => { self.regs.b = self.fetch(bus); 2 }
            0x0E => { self.regs.c = self.fetch(bus); 2 }
            0x16 => { self.regs.d = self.fetch(bus); 2 }
            0x1E => { self.regs.e = self.fetch(bus); 2 }
            0x26 => { self.regs.h = self.fetch(bus); 2 }
            0x2E => { self.regs.l = self.fetch(bus); 2 }
            0x36 => { let v = self.fetch(bus); bus.write(self.regs.get_hl(), v); 3 }
            
            // LD A, (BC/DE) y Stores
            0x02 => { bus.write(self.regs.get_bc(), self.regs.a); 2 }
            0x12 => { bus.write(self.regs.get_de(), self.regs.a); 2 }
            0x0A => { self.regs.a = bus.read(self.regs.get_bc()); 2 }
            0x1A => { self.regs.a = bus.read(self.regs.get_de()); 2 }

            // LDI / LDD (Load Increment/Decrement)
            0x22 => { bus.write(self.regs.get_hl(), self.regs.a); self.regs.set_hl(self.regs.get_hl().wrapping_add(1)); 2 }
            0x2A => { self.regs.a = bus.read(self.regs.get_hl()); self.regs.set_hl(self.regs.get_hl().wrapping_add(1)); 2 }
            0x32 => { bus.write(self.regs.get_hl(), self.regs.a); self.regs.set_hl(self.regs.get_hl().wrapping_sub(1)); 2 }
            0x3A => { self.regs.a = bus.read(self.regs.get_hl()); self.regs.set_hl(self.regs.get_hl().wrapping_sub(1)); 2 }

            // Cargas Inmediatas 0x3E (LD A, d8)
            0x3E => { self.regs.a = self.fetch(bus); 2 }

            // LD r, r (0x40 - 0x7F) - Excluyendo HALT (0x76)
            0x40..=0x7F => {
                if opcode == 0x76 { self.halted = true; 1 }
                else { self.execute_load_8bit(opcode, bus) }
            }

            // --- GRUPO 2: Cargas de 16 bits ---
            0x01 => { let v = self.fetch_u16(bus); self.regs.set_bc(v); 3 }
            0x11 => { let v = self.fetch_u16(bus); self.regs.set_de(v); 3 }
            0x21 => { let v = self.fetch_u16(bus); self.regs.set_hl(v); 3 }
            0x31 => { self.regs.sp = self.fetch_u16(bus); 3 }
            // LD HL, SP+r8
            // Suma un valor con signo al SP y lo guarda en HL.
            // Afecta flags H y C (Zero y Subtract siempre son 0).
            0xF8 => {
                let offset = self.fetch(bus) as i8; // Leemos el byte como entero con signo
                let sp = self.regs.sp;
                
                // Calculamos el resultado (SP + offset)
                // Hacemos cast a i16 para mantener el signo, y luego a u16 para la suma
                let res = sp.wrapping_add(offset as i16 as u16);
                
                self.regs.set_hl(res);
                
                self.regs.f = 0; // Z y N siempre se resetean a 0
                
                // Cálculo de Flags (Es "tricky": se basa en desbordamiento de bits bajos)
                // Half Carry: Desbordamiento del bit 3
                if (sp & 0xF) + (offset as u16 & 0xF) > 0xF { 
                    self.regs.f |= H_FLAG; 
                }
                
                // Carry: Desbordamiento del bit 7
                if (sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF { 
                    self.regs.f |= C_FLAG; 
                }
                
                3 // Toma 3 M-Cycles (12 T-Cycles)
            }
            
            0xF9 => { self.regs.sp = self.regs.get_hl(); 2 } // LD SP, HL
            0x08 => { // LD (a16), SP
                let addr = self.fetch_u16(bus);
                let sp = self.regs.sp;
                bus.write(addr, (sp & 0xFF) as u8);
                bus.write(addr.wrapping_add(1), (sp >> 8) as u8);
                5
            }
            
            // PUSH / POP
            0xC1 => { let v = self.pop(bus); self.regs.set_bc(v); 3 }
            0xD1 => { let v = self.pop(bus); self.regs.set_de(v); 3 }
            0xE1 => { let v = self.pop(bus); self.regs.set_hl(v); 3 }
            0xF1 => { let v = self.pop(bus); self.regs.set_af(v); 3 }
            0xC5 => { self.push(bus, self.regs.get_bc()); 4 }
            0xD5 => { self.push(bus, self.regs.get_de()); 4 }
            0xE5 => { self.push(bus, self.regs.get_hl()); 4 }
            0xF5 => { self.push(bus, self.regs.get_af()); 4 }

            // --- GRUPO 3: Aritmética 8 bits ---
            0x04 => { self.regs.b = self.inc(self.regs.b); 1 }
            0x05 => { self.regs.b = self.dec(self.regs.b); 1 }
            0x0C => { self.regs.c = self.inc(self.regs.c); 1 }
            0x0D => { self.regs.c = self.dec(self.regs.c); 1 }
            0x14 => { self.regs.d = self.inc(self.regs.d); 1 }
            0x15 => { self.regs.d = self.dec(self.regs.d); 1 }
            0x1C => { self.regs.e = self.inc(self.regs.e); 1 }
            0x1D => { self.regs.e = self.dec(self.regs.e); 1 }
            0x24 => { self.regs.h = self.inc(self.regs.h); 1 }
            0x25 => { self.regs.h = self.dec(self.regs.h); 1 }
            0x2C => { self.regs.l = self.inc(self.regs.l); 1 }
            0x2D => { self.regs.l = self.dec(self.regs.l); 1 }
            0x3C => { self.regs.a = self.inc(self.regs.a); 1 }
            0x3D => { self.regs.a = self.dec(self.regs.a); 1 }
            0x34 => { // INC (HL)
                let addr = self.regs.get_hl();
                let v = bus.read(addr);
                bus.write(addr, self.inc(v));
                3
            }
            0x35 => { // DEC (HL)
                let addr = self.regs.get_hl();
                let v = bus.read(addr);
                bus.write(addr, self.dec(v));
                3
            }

            // Operaciones ALU con registro (ADD, ADC, SUB, SBC, AND, XOR, OR, CP)
            0x80..=0x87 => { self.add(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0x88..=0x8F => { self.adc(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0x90..=0x97 => { self.sub(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0x98..=0x9F => { self.sbc(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0xA0..=0xA7 => { self.and(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0xA8..=0xAF => { self.xor(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0xB0..=0xB7 => { self.or(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0xB8..=0xBF => { self.cp(self.get_reg_from_code(opcode & 0x07, bus)); 1 }

            // Operaciones ALU Inmediatas (d8)
            0xC6 => { let v = self.fetch(bus); self.add(v); 2 }
            0xCE => { let v = self.fetch(bus); self.adc(v); 2 }
            0xD6 => { let v = self.fetch(bus); self.sub(v); 2 }
            0xDE => { let v = self.fetch(bus); self.sbc(v); 2 }
            0xE6 => { let v = self.fetch(bus); self.and(v); 2 }
            0xEE => { let v = self.fetch(bus); self.xor(v); 2 }
            0xF6 => { let v = self.fetch(bus); self.or(v); 2 }
            0xFE => { let v = self.fetch(bus); self.cp(v); 2 }

            // Aritmética 16 bits (ADD HL, rr) y (INC/DEC rr)
            0x09 => { self.add_hl(self.regs.get_bc()); 2 }
            0x19 => { self.add_hl(self.regs.get_de()); 2 }
            0x29 => { self.add_hl(self.regs.get_hl()); 2 }
            0x39 => { self.add_hl(self.regs.sp); 2 }
            
            0x03 => { self.regs.set_bc(self.regs.get_bc().wrapping_add(1)); 2 }
            0x13 => { self.regs.set_de(self.regs.get_de().wrapping_add(1)); 2 }
            0x23 => { self.regs.set_hl(self.regs.get_hl().wrapping_add(1)); 2 }
            0x33 => { self.regs.sp = self.regs.sp.wrapping_add(1); 2 }

            0x0B => { self.regs.set_bc(self.regs.get_bc().wrapping_sub(1)); 2 }
            0x1B => { self.regs.set_de(self.regs.get_de().wrapping_sub(1)); 2 }
            0x2B => { self.regs.set_hl(self.regs.get_hl().wrapping_sub(1)); 2 }
            0x3B => { self.regs.sp = self.regs.sp.wrapping_sub(1); 2 }

            // --- GRUPO 4: Saltos ---
            0xC3 => { self.regs.pc = self.fetch_u16(bus); 4 } // JP a16
            0xE9 => { self.regs.pc = self.regs.get_hl(); 1 }  // JP (HL)
            
            0x18 => { self.jr(bus, true) }
            0x20 => { self.jr(bus, !self.get_flag(Z_FLAG)) }
            0x28 => { self.jr(bus, self.get_flag(Z_FLAG)) }
            0x30 => { self.jr(bus, !self.get_flag(C_FLAG)) }
            0x38 => { self.jr(bus, self.get_flag(C_FLAG)) }

            0xC2 => { self.jp(bus, !self.get_flag(Z_FLAG)) }
            0xCA => { self.jp(bus, self.get_flag(Z_FLAG)) }
            0xD2 => { self.jp(bus, !self.get_flag(C_FLAG)) }
            0xDA => { self.jp(bus, self.get_flag(C_FLAG)) }

            0xCD => { self.call(bus, true) }
            0xC4 => { self.call(bus, !self.get_flag(Z_FLAG)) }
            0xCC => { self.call(bus, self.get_flag(Z_FLAG)) }
            0xD4 => { self.call(bus, !self.get_flag(C_FLAG)) }
            0xDC => { self.call(bus, self.get_flag(C_FLAG)) }

            0xC9 => { self.ret(bus, true) }
            0xC0 => { self.ret(bus, !self.get_flag(Z_FLAG)) }
            0xC8 => { self.ret(bus, self.get_flag(Z_FLAG)) }
            0xD0 => { self.ret(bus, !self.get_flag(C_FLAG)) }
            0xD8 => { self.ret(bus, self.get_flag(C_FLAG)) }
            0xD9 => { self.regs.pc = self.pop(bus); self.ime = true; 4 } // RETI

            // RST (Restarts)
            0xC7 => { self.rst(bus, 0x00); 4 }
            0xCF => { self.rst(bus, 0x08); 4 }
            0xD7 => { self.rst(bus, 0x10); 4 }
            0xDF => { self.rst(bus, 0x18); 4 }
            0xE7 => { self.rst(bus, 0x20); 4 }
            0xEF => { self.rst(bus, 0x28); 4 }
            0xF7 => { self.rst(bus, 0x30); 4 }
            0xFF => { self.rst(bus, 0x38); 4 }

            // --- GRUPO 5: Rotaciones de A (Distinctas de CB) ---
            0x07 => { // RLCA
                let val = self.regs.a;
                let carry = (val & 0x80) >> 7;
                self.regs.a = (val << 1) | carry;
                self.regs.f = 0; // En SM83 RLCA siempre pone Z=0
                if carry != 0 { self.regs.f |= C_FLAG; }
                1
            }
            0x0F => { // RRCA
                let val = self.regs.a;
                let carry = val & 0x01;
                self.regs.a = (val >> 1) | (carry << 7);
                self.regs.f = 0;
                if carry != 0 { self.regs.f |= C_FLAG; }
                1
            }
            0x17 => { // RLA
                let val = self.regs.a;
                let old_carry = if self.get_flag(C_FLAG) { 1 } else { 0 };
                let new_carry = (val & 0x80) >> 7;
                self.regs.a = (val << 1) | old_carry;
                self.regs.f = 0;
                if new_carry != 0 { self.regs.f |= C_FLAG; }
                1
            }
            0x1F => { // RRA
                let val = self.regs.a;
                let old_carry = if self.get_flag(C_FLAG) { 1 } else { 0 };
                let new_carry = val & 0x01;
                self.regs.a = (val >> 1) | (old_carry << 7);
                self.regs.f = 0;
                if new_carry != 0 { self.regs.f |= C_FLAG; }
                1
            }

            // --- GRUPO 6: Misceláneos ---
            0x27 => { self.daa(); 1 } // DAA (Decimal Adjust)
            0x2F => { self.regs.a = !self.regs.a; self.set_flag(N_FLAG, true); self.set_flag(H_FLAG, true); 1 } // CPL
            0x37 => { self.set_flag(N_FLAG, false); self.set_flag(H_FLAG, false); self.set_flag(C_FLAG, true); 1 } // SCF
            0x3F => { self.set_flag(N_FLAG, false); self.set_flag(H_FLAG, false); let c = self.get_flag(C_FLAG); self.set_flag(C_FLAG, !c); 1 } // CCF
            0xF3 => { self.ime = false; 1 } // DI
            0xFB => { self.ime = true; 1 } // EI

            // High RAM ops
            0xE0 => { let off = self.fetch(bus) as u16; bus.write(0xFF00 + off, self.regs.a); 3 } // LDH (a8), A
            0xF0 => { let off = self.fetch(bus) as u16; self.regs.a = bus.read(0xFF00 + off); 3 } // LDH A, (a8)
            0xE2 => { bus.write(0xFF00 + (self.regs.c as u16), self.regs.a); 2 } // LD (C), A
            0xF2 => { self.regs.a = bus.read(0xFF00 + (self.regs.c as u16)); 2 } // LD A, (C)
            0xEA => { let addr = self.fetch_u16(bus); bus.write(addr, self.regs.a); 4 } // LD (a16), A
            0xFA => { let addr = self.fetch_u16(bus); self.regs.a = bus.read(addr); 4 } // LD A, (a16)

            // Añadir Add SP, e8 (0xE8) y LD HL, SP+e8 (0xF8) si es necesario, son un poco complejos.
            // Por simplicidad en Tetris a veces no se usan, pero es bueno tenerlos en cuenta.
            0xE8 => { // ADD SP, r8
                let offset = self.fetch(bus) as i8 as u16;
                let sp = self.regs.sp;
                let res = sp.wrapping_add(offset);
                self.regs.f = 0;
                // Flags H y C se calculan con los bits bajos (byte 0)
                if (sp & 0xF) + (offset & 0xF) > 0xF { self.regs.f |= H_FLAG; }
                if (sp & 0xFF) + (offset & 0xFF) > 0xFF { self.regs.f |= C_FLAG; }
                self.regs.sp = res;
                4
            }

            0xCB => { self.execute_cb(bus) }
            
            _ => { 1 } // Opcodes no definidos tratados como NOP por seguridad
        }
    }

    // --- PREFIX CB ---
    fn execute_cb(&mut self, bus: &mut Bus) -> u32 {
        let opcode = self.fetch(bus);
        let reg_idx = opcode & 0x07;
        let mut val = self.get_reg_from_code(reg_idx, bus);
        let cycles = if reg_idx == 6 { 4 } else { 2 };

        match opcode {
            0x00..=0x07 => { // RLC
                let carry = (val & 0x80) >> 7;
                val = (val << 1) | carry;
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if carry != 0 { self.regs.f |= C_FLAG; }
            }
            0x08..=0x0F => { // RRC
                let carry = val & 0x01;
                val = (val >> 1) | (carry << 7);
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if carry != 0 { self.regs.f |= C_FLAG; }
            }
            0x10..=0x17 => { // RL
                let old_c = if self.get_flag(C_FLAG) { 1 } else { 0 };
                let new_c = (val & 0x80) >> 7;
                val = (val << 1) | old_c;
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if new_c != 0 { self.regs.f |= C_FLAG; }
            }
            0x18..=0x1F => { // RR
                let old_c = if self.get_flag(C_FLAG) { 1 } else { 0 };
                let new_c = val & 0x01;
                val = (val >> 1) | (old_c << 7);
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if new_c != 0 { self.regs.f |= C_FLAG; }
            }
            0x20..=0x27 => { // SLA
                let c = (val & 0x80) >> 7;
                val <<= 1;
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if c != 0 { self.regs.f |= C_FLAG; }
            }
            0x28..=0x2F => { // SRA
                let c = val & 0x01;
                val = (val as i8 >> 1) as u8; // Aritmético: Mantiene signo
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if c != 0 { self.regs.f |= C_FLAG; }
            }
            0x30..=0x37 => { // SWAP
                val = (val << 4) | (val >> 4);
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; }
            }
            0x38..=0x3F => { // SRL
                let c = val & 0x01;
                val >>= 1;
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if c != 0 { self.regs.f |= C_FLAG; }
            }
            0x40..=0x7F => { // BIT
                let bit = (opcode >> 3) & 0x07;
                let zero = (val & (1 << bit)) == 0;
                self.set_flag(Z_FLAG, zero);
                self.set_flag(N_FLAG, false);
                self.set_flag(H_FLAG, true);
                return if reg_idx == 6 { 3 } else { 2 };
            }
            0x80..=0xBF => { // RES
                val &= !(1 << ((opcode >> 3) & 0x07));
            }
            0xC0..=0xFF => { // SET
                val |= 1 << ((opcode >> 3) & 0x07);
            }
        }

        self.write_reg_cb(bus, reg_idx, val);
        cycles
    }

    // --- ALU HELPERS ---

    fn add(&mut self, val: u8) {
        let (res, carry) = self.regs.a.overflowing_add(val);
        let half = (self.regs.a & 0xF) + (val & 0xF) > 0xF;
        self.regs.a = res;
        self.regs.f = 0;
        if res == 0 { self.regs.f |= Z_FLAG; }
        if half { self.regs.f |= H_FLAG; }
        if carry { self.regs.f |= C_FLAG; }
    }

    fn adc(&mut self, val: u8) {
        let c = if self.get_flag(C_FLAG) { 1 } else { 0 };
        let (res1, carry1) = self.regs.a.overflowing_add(val);
        let (res2, carry2) = res1.overflowing_add(c);
        let half = (self.regs.a & 0xF) + (val & 0xF) + c > 0xF;
        self.regs.a = res2;
        self.regs.f = 0;
        if res2 == 0 { self.regs.f |= Z_FLAG; }
        if half { self.regs.f |= H_FLAG; }
        if carry1 || carry2 { self.regs.f |= C_FLAG; }
    }

    fn sub(&mut self, val: u8) {
        let (res, carry) = self.regs.a.overflowing_sub(val);
        let half = (self.regs.a & 0xF) < (val & 0xF);
        self.regs.a = res;
        self.regs.f = N_FLAG;
        if res == 0 { self.regs.f |= Z_FLAG; }
        if half { self.regs.f |= H_FLAG; }
        if carry { self.regs.f |= C_FLAG; }
    }

    fn sbc(&mut self, val: u8) {
        let c = if self.get_flag(C_FLAG) { 1 } else { 0 };
        let (res1, carry1) = self.regs.a.overflowing_sub(val);
        let (res2, carry2) = res1.overflowing_sub(c);
        let half = (self.regs.a & 0xF) < (val & 0xF) + c; 
        self.regs.a = res2;
        self.regs.f = N_FLAG;
        if res2 == 0 { self.regs.f |= Z_FLAG; }
        if half { self.regs.f |= H_FLAG; }
        if carry1 || carry2 { self.regs.f |= C_FLAG; }
    }

    fn and(&mut self, val: u8) { self.regs.a &= val; self.regs.f = H_FLAG; if self.regs.a == 0 { self.regs.f |= Z_FLAG; } }
    fn or(&mut self, val: u8) { self.regs.a |= val; self.regs.f = 0; if self.regs.a == 0 { self.regs.f |= Z_FLAG; } }
    fn xor(&mut self, val: u8) { self.regs.a ^= val; self.regs.f = 0; if self.regs.a == 0 { self.regs.f |= Z_FLAG; } }
    fn cp(&mut self, val: u8) {
        let (res, carry) = self.regs.a.overflowing_sub(val);
        let half = (self.regs.a & 0xF) < (val & 0xF);
        self.regs.f = N_FLAG;
        if res == 0 { self.regs.f |= Z_FLAG; }
        if half { self.regs.f |= H_FLAG; }
        if carry { self.regs.f |= C_FLAG; }
    }

    fn inc(&mut self, val: u8) -> u8 {
        let res = val.wrapping_add(1);
        self.set_flag(Z_FLAG, res == 0);
        self.set_flag(N_FLAG, false);
        self.set_flag(H_FLAG, (val & 0xF) == 0xF);
        res
    }

    fn dec(&mut self, val: u8) -> u8 {
        let res = val.wrapping_sub(1);
        self.set_flag(Z_FLAG, res == 0);
        self.set_flag(N_FLAG, true);
        self.set_flag(H_FLAG, (val & 0xF) == 0);
        res
    }

    fn add_hl(&mut self, val: u16) {
        let hl = self.regs.get_hl();
        let (res, carry) = hl.overflowing_add(val);
        let half = (hl & 0xFFF) + (val & 0xFFF) > 0xFFF;
        self.regs.set_hl(res);
        self.set_flag(N_FLAG, false);
        self.set_flag(H_FLAG, half);
        self.set_flag(C_FLAG, carry);
    }

    // Decimal Adjust Accumulator (El monstruo final de la emulación de GB)
    fn daa(&mut self) {
        let mut a = self.regs.a;
        let mut adjust = 0;
        if self.get_flag(H_FLAG) || (!self.get_flag(N_FLAG) && (a & 0xF) > 9) {
            adjust |= 0x06;
        }
        if self.get_flag(C_FLAG) || (!self.get_flag(N_FLAG) && a > 0x99) {
            adjust |= 0x60;
            self.set_flag(C_FLAG, true);
        }
        if self.get_flag(N_FLAG) {
            a = a.wrapping_sub(adjust);
        } else {
            a = a.wrapping_add(adjust);
        }
        self.regs.a = a;
        self.set_flag(Z_FLAG, a == 0);
        self.set_flag(H_FLAG, false);
    }

    // --- MEMORY HELPERS ---

    fn fetch(&mut self, bus: &mut Bus) -> u8 {
        let v = bus.read(self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        v
    }

    fn fetch_u16(&mut self, bus: &mut Bus) -> u16 {
        let l = self.fetch(bus) as u16;
        let h = self.fetch(bus) as u16;
        (h << 8) | l
    }

    fn push(&mut self, bus: &mut Bus, val: u16) {
        self.regs.sp = self.regs.sp.wrapping_sub(1); bus.write(self.regs.sp, (val >> 8) as u8);
        self.regs.sp = self.regs.sp.wrapping_sub(1); bus.write(self.regs.sp, val as u8);
    }

    fn pop(&mut self, bus: &mut Bus) -> u16 {
        let l = bus.read(self.regs.sp) as u16; self.regs.sp = self.regs.sp.wrapping_add(1);
        let h = bus.read(self.regs.sp) as u16; self.regs.sp = self.regs.sp.wrapping_add(1);
        (h << 8) | l
    }

    // --- CONTROL FLOW ---

    fn call(&mut self, bus: &mut Bus, cond: bool) -> u32 {
        let addr = self.fetch_u16(bus);
        if cond {
            self.push(bus, self.regs.pc);
            self.regs.pc = addr;
            6
        } else { 3 }
    }

    fn ret(&mut self, bus: &mut Bus, cond: bool) -> u32 {
        if cond {
            self.regs.pc = self.pop(bus);
            // Ret toma más ciclos si es condicional tomado (5) vs incondicional (4)
            // Aquí simplificamos a 4/5 para mantener el flujo
            4 
        } else { 2 }
    }

    fn jp(&mut self, bus: &mut Bus, cond: bool) -> u32 {
        let addr = self.fetch_u16(bus);
        if cond { self.regs.pc = addr; 4 } else { 3 }
    }

    fn jr(&mut self, bus: &mut Bus, cond: bool) -> u32 {
        let off = self.fetch(bus) as i8;
        if cond {
            self.regs.pc = (self.regs.pc as i32 + off as i32) as u16;
            3
        } else { 2 }
    }

    fn rst(&mut self, bus: &mut Bus, addr: u16) {
        self.push(bus, self.regs.pc);
        self.regs.pc = addr;
    }

    // --- UTILS ---

    fn get_flag(&self, f: u8) -> bool { (self.regs.f & f) != 0 }
    fn set_flag(&mut self, f: u8, v: bool) { if v { self.regs.f |= f; } else { self.regs.f &= !f; } }

    fn execute_load_8bit(&mut self, opcode: u8, bus: &mut Bus) -> u32 {
        let src = opcode & 0x07;
        let dst = (opcode >> 3) & 0x07;
        let val = self.get_reg_from_code(src, bus);
        match dst {
            0 => self.regs.b = val, 1 => self.regs.c = val,
            2 => self.regs.d = val, 3 => self.regs.e = val,
            4 => self.regs.h = val, 5 => self.regs.l = val,
            6 => bus.write(self.regs.get_hl(), val),
            7 => self.regs.a = val,
            _ => {}
        }
        if src == 6 || dst == 6 { 2 } else { 1 }
    }

    fn get_reg_from_code(&self, code: u8, bus: &Bus) -> u8 {
        match code {
            0 => self.regs.b, 1 => self.regs.c,
            2 => self.regs.d, 3 => self.regs.e,
            4 => self.regs.h, 5 => self.regs.l,
            6 => bus.read(self.regs.get_hl()),
            7 => self.regs.a,
            _ => 0,
        }
    }

    fn write_reg_cb(&mut self, bus: &mut Bus, idx: u8, val: u8) {
        match idx {
            0 => self.regs.b = val, 1 => self.regs.c = val,
            2 => self.regs.d = val, 3 => self.regs.e = val,
            4 => self.regs.h = val, 5 => self.regs.l = val,
            6 => bus.write(self.regs.get_hl(), val),
            7 => self.regs.a = val,
            _ => {}
        }
    }
}