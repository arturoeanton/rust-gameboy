// cpu/src/lib.rs
use memory::Bus;

/// Banderas del registro F (Flags).
/// En Game Boy, el registro F contiene 4 bits de estado que las instrucciones consultan.
const Z_FLAG: u8 = 0x80; // Zero: Resultado fue 0 (Bit 7)
const N_FLAG: u8 = 0x40; // Subtraction: La última operación fue resta (Bit 6)
const H_FLAG: u8 = 0x20; // Half Carry: Acarreo del bit 3 al 4 (Bit 5). Usado para ajuste BCD (DAA).
const C_FLAG: u8 = 0x10; // Carry: Acarreo del bit 7 (Overflow) (Bit 4)

/// Estructura de Registros del CPU.
/// Rust struct memory layout es predecible (aunque no garantizado sin #[repr(C)]).
/// Aquí guardamos todos los valores de 8 y 16 bits.
pub struct Registers {
    pub a: u8, pub f: u8, // AF
    pub b: u8, pub c: u8, // BC
    pub d: u8, pub e: u8, // DE
    pub h: u8, pub l: u8, // HL (Puntero de memoria principal)
    pub sp: u16,          // Stack Pointer
    pub pc: u16,          // Program Counter
}

impl Registers {
    pub fn new() -> Self {
        // Valores iniciales (Post-Bootrom).
        // Estos valores mágicos son el estado en el que la BOOTROM deja la CPU al terminar.
        Self {
            a: 0x01, f: 0xB0,
            b: 0x00, c: 0x13,
            d: 0x00, e: 0xD8,
            h: 0x01, l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100, // Punto de entrada de los cartuchos (Nintendo Logo check va antes)
        }
    }

    // Helpers para pares de 16 bits (Virtual Registers).
    // En Rust, usamos métodos getters/setters para combinar dos u8 en un u16.
    // 'val >> 8' mueve los bits altos a la posición baja.
    // 'val as u8' trunca los bits altos (cast destructivo seguro).
    pub fn get_bc(&self) -> u16 { (self.b as u16) << 8 | (self.c as u16) }
    pub fn set_bc(&mut self, val: u16) { self.b = (val >> 8) as u8; self.c = val as u8; }

    pub fn get_de(&self) -> u16 { (self.d as u16) << 8 | (self.e as u16) }
    pub fn set_de(&mut self, val: u16) { self.d = (val >> 8) as u8; self.e = val as u8; }

    pub fn get_hl(&self) -> u16 { (self.h as u16) << 8 | (self.l as u16) }
    pub fn set_hl(&mut self, val: u16) { self.h = (val >> 8) as u8; self.l = val as u8; }

    // AF es especial: Los 4 bits bajos de F siempre deben ser 0 en hardware real.
    pub fn get_af(&self) -> u16 { (self.a as u16) << 8 | (self.f as u16) }
    pub fn set_af(&mut self, val: u16) { self.a = (val >> 8) as u8; self.f = (val as u8) & 0xF0; }
}

/// Estado global del CPU
pub struct Cpu {
    pub regs: Registers,
    pub ime: bool,    // Interrupt Master Enable (Switch global de interrupciones)
    pub halted: bool, // Modo de bajo consumo (HALT instruction)
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: Registers::new(),
            ime: false,
            halted: false,
        }
    }

    /// Ciclo principal: Fetch, Decode, Execute.
    /// Retorna el número de ciclos de máquina (M-Cycles) consumidos.
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        // 1. Verificar si estamos en modo HALT
        if self.halted {
            // Si hay interrupción pendiente, despertamos.
            if self.ime && (bus.interrupt_flag & bus.interrupt_enable & 0x1F) != 0 {
                self.halted = false;
            } else {
                return 1; // CPU dormida, consume 1 ciclo sin hacer nada.
            }
        }
        
        // 2. Manejo de INTERRUPCIONES (Hardware Interrupts)
        if self.ime {
            // Un bit en 1 en ambos (Flag y Enable) dispara la interrupción.
            let fired = bus.interrupt_flag & bus.interrupt_enable;
            
            if (fired & 0x1F) != 0 {
                self.ime = false; // Deshabilitar interrupciones para evitar reentrancia infinita
                self.halted = false;

                // Push PC: Guardamos dirección de retorno en el stack
                self.push(bus, self.regs.pc);

                // Priority Check hardcoded (hardware fixed priority)
                if (fired & 0x01) != 0 {      // V-Blank
                    self.regs.pc = 0x0040;
                    bus.interrupt_flag &= !0x01;
                } else if (fired & 0x02) != 0 { // LCD Stat
                    self.regs.pc = 0x0048;
                    bus.interrupt_flag &= !0x02;
                } else if (fired & 0x04) != 0 { // Timer
                    self.regs.pc = 0x0050;
                    bus.interrupt_flag &= !0x04;
                } else if (fired & 0x08) != 0 { // Serial
                    self.regs.pc = 0x0058;
                    bus.interrupt_flag &= !0x08;
                } else if (fired & 0x10) != 0 { // Joypad
                    self.regs.pc = 0x0060;
                    bus.interrupt_flag &= !0x10;
                }
                
                return 5; // ISR Dispatch toma 5 M-Cycles
            }
        }

        // 3. FETCH: Leer opcode
        let opcode = self.fetch(bus);

        // 4. DECODE & EXECUTE: El gran match de Rust
        match opcode {
            // --- NOP & Control ---
            0x00 => { 1 } // NOP
            0x10 => { self.fetch(bus); 1 } // STOP (ignora siguiente byte)
            0x76 => { self.halted = true; 1 } // HALT

            // --- Cargas de 8 bits (Load) ---
            0x06 => { self.regs.b = self.fetch(bus); 2 } // LD B, n
            0x0E => { self.regs.c = self.fetch(bus); 2 }
            0x16 => { self.regs.d = self.fetch(bus); 2 } // LD D, n
            0x1E => { self.regs.e = self.fetch(bus); 2 }
            0x26 => { self.regs.h = self.fetch(bus); 2 } // LD H, n
            0x2E => { self.regs.l = self.fetch(bus); 2 }
            
            // LD (HL), n: Escribir en memoria apuntada por HL
            0x36 => { let v = self.fetch(bus); bus.write(self.regs.get_hl(), v); 3 }
            
            // Stores y Loads indirectos
            0x02 => { bus.write(self.regs.get_bc(), self.regs.a); 2 } // LD (BC), A
            0x12 => { bus.write(self.regs.get_de(), self.regs.a); 2 } // LD (DE), A
            0x0A => { self.regs.a = bus.read(self.regs.get_bc()); 2 } // LD A, (BC)
            0x1A => { self.regs.a = bus.read(self.regs.get_de()); 2 } // LD A, (DE)

            // LDI / LDD: Load and Increment/Decrement HL
            0x22 => { bus.write(self.regs.get_hl(), self.regs.a); self.regs.set_hl(self.regs.get_hl().wrapping_add(1)); 2 }
            0x2A => { self.regs.a = bus.read(self.regs.get_hl()); self.regs.set_hl(self.regs.get_hl().wrapping_add(1)); 2 }
            0x32 => { bus.write(self.regs.get_hl(), self.regs.a); self.regs.set_hl(self.regs.get_hl().wrapping_sub(1)); 2 }
            0x3A => { self.regs.a = bus.read(self.regs.get_hl()); self.regs.set_hl(self.regs.get_hl().wrapping_sub(1)); 2 }

            0x3E => { self.regs.a = self.fetch(bus); 2 } // LD A, n

            // LD r, r (Cargas registro a registro)
            // Agrupamos el rango 0x40-0x7F y manejamos la excepción de HALT (0x76)
            0x40..=0x7F => {
                if opcode == 0x76 { self.halted = true; 1 }
                else { self.execute_load_8bit(opcode, bus) }
            }

            // --- Cargas de 16 bits ---
            0x01 => { let v = self.fetch_u16(bus); self.regs.set_bc(v); 3 }
            0x11 => { let v = self.fetch_u16(bus); self.regs.set_de(v); 3 }
            0x21 => { let v = self.fetch_u16(bus); self.regs.set_hl(v); 3 }
            0x31 => { self.regs.sp = self.fetch_u16(bus); 3 } // LD SP, nn
            
            // LD HL, SP+r8: Aritmética de punteros compleja
            0xF8 => {
                let offset = self.fetch(bus) as i8; // Cast a signed
                let sp = self.regs.sp;
                let res = sp.wrapping_add(offset as i16 as u16);
                self.regs.set_hl(res);
                self.regs.f = 0;
                // Flags H y C funcionan raro con SP aritmetica (base 16 bits, flags 8 bits)
                if (sp & 0xF) + (offset as u16 & 0xF) > 0xF { self.regs.f |= H_FLAG; }
                if (sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF { self.regs.f |= C_FLAG; }
                3
            }
            
            0xF9 => { self.regs.sp = self.regs.get_hl(); 2 } // LD SP, HL
            
            0x08 => { // LD (nn), SP
                let addr = self.fetch_u16(bus);
                let sp = self.regs.sp;
                bus.write(addr, (sp & 0xFF) as u8);
                bus.write(addr.wrapping_add(1), (sp >> 8) as u8);
                5
            }
            
            // PUSH (Stack)
            0xC5 => { self.push(bus, self.regs.get_bc()); 4 }
            0xD5 => { self.push(bus, self.regs.get_de()); 4 }
            0xE5 => { self.push(bus, self.regs.get_hl()); 4 }
            0xF5 => { self.push(bus, self.regs.get_af()); 4 }
            // POP
            0xC1 => { let v = self.pop(bus); self.regs.set_bc(v); 3 }
            0xD1 => { let v = self.pop(bus); self.regs.set_de(v); 3 }
            0xE1 => { let v = self.pop(bus); self.regs.set_hl(v); 3 }
            0xF1 => { let v = self.pop(bus); self.regs.set_af(v); 3 }

            // --- ALU 8 bits (Aritmética) ---
            // INC / DEC (Afectan Z, N, H. NO afectan C)
            0x04 => { self.regs.b = self.inc(self.regs.b); 1 }
            0x05 => { self.regs.b = self.dec(self.regs.b); 1 }
            0x0C => { self.regs.c = self.inc(self.regs.c); 1 }
            0x0D => { self.regs.c = self.dec(self.regs.c); 1 }
            // ... (Repeticiones para D, E, H, L, A omitidas por brevedad, ver implementación completa)
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

            // Operaciones ALU lógicas y aritméticas con acumulador (A)
            // 0x80 - 0xBF
            0x80..=0x87 => { self.add(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0x88..=0x8F => { self.adc(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0x90..=0x97 => { self.sub(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0x98..=0x9F => { self.sbc(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0xA0..=0xA7 => { self.and(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0xA8..=0xAF => { self.xor(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0xB0..=0xB7 => { self.or(self.get_reg_from_code(opcode & 0x07, bus)); 1 }
            0xB8..=0xBF => { self.cp(self.get_reg_from_code(opcode & 0x07, bus)); 1 }

            // Operaciones ALU Inmediatas (n)
            0xC6 => { let v = self.fetch(bus); self.add(v); 2 }
            0xCE => { let v = self.fetch(bus); self.adc(v); 2 }
            0xD6 => { let v = self.fetch(bus); self.sub(v); 2 }
            0xDE => { let v = self.fetch(bus); self.sbc(v); 2 }
            0xE6 => { let v = self.fetch(bus); self.and(v); 2 }
            0xEE => { let v = self.fetch(bus); self.xor(v); 2 }
            0xF6 => { let v = self.fetch(bus); self.or(v); 2 }
            0xFE => { let v = self.fetch(bus); self.cp(v); 2 }

            // ALU 16 bits (ADD HL, rr)
            0x09 => { self.add_hl(self.regs.get_bc()); 2 }
            0x19 => { self.add_hl(self.regs.get_de()); 2 }
            0x29 => { self.add_hl(self.regs.get_hl()); 2 }
            0x39 => { self.add_hl(self.regs.sp); 2 }

            // INC/DEC 16 bits (Note: Flags NO cambian)
            0x03 => { self.regs.set_bc(self.regs.get_bc().wrapping_add(1)); 2 }
            0x13 => { self.regs.set_de(self.regs.get_de().wrapping_add(1)); 2 }
            0x23 => { self.regs.set_hl(self.regs.get_hl().wrapping_add(1)); 2 }
            0x33 => { self.regs.sp = self.regs.sp.wrapping_add(1); 2 }
            0x0B => { self.regs.set_bc(self.regs.get_bc().wrapping_sub(1)); 2 }
            0x1B => { self.regs.set_de(self.regs.get_de().wrapping_sub(1)); 2 }
            0x2B => { self.regs.set_hl(self.regs.get_hl().wrapping_sub(1)); 2 }
            0x3B => { self.regs.sp = self.regs.sp.wrapping_sub(1); 2 }

            // --- Saltos (Control Flow) ---
            0xC3 => { self.regs.pc = self.fetch_u16(bus); 4 } // JP nn
            0xE9 => { self.regs.pc = self.regs.get_hl(); 1 }  // JP (HL)
            
            // Saltos Relativos (JR)
            0x18 => { self.jr(bus, true) }
            0x20 => { self.jr(bus, !self.get_flag(Z_FLAG)) } // JR NZ, n
            0x28 => { self.jr(bus, self.get_flag(Z_FLAG)) }  // JR Z, n
            0x30 => { self.jr(bus, !self.get_flag(C_FLAG)) } // JR NC, n
            0x38 => { self.jr(bus, self.get_flag(C_FLAG)) }  // JR C, n

            // Saltos Absolutos (JP condicional)
            0xC2 => { self.jp(bus, !self.get_flag(Z_FLAG)) }
            0xCA => { self.jp(bus, self.get_flag(Z_FLAG)) }
            0xD2 => { self.jp(bus, !self.get_flag(C_FLAG)) }
            0xDA => { self.jp(bus, self.get_flag(C_FLAG)) }

            // Calls
            0xCD => { self.call(bus, true) } // CALL nn
            0xC4 => { self.call(bus, !self.get_flag(Z_FLAG)) }
            0xCC => { self.call(bus, self.get_flag(Z_FLAG)) }
            0xD4 => { self.call(bus, !self.get_flag(C_FLAG)) }
            0xDC => { self.call(bus, self.get_flag(C_FLAG)) }

            // Returns
            0xC9 => { self.ret(bus, true) } // RET
            0xC0 => { self.ret(bus, !self.get_flag(Z_FLAG)) }
            0xC8 => { self.ret(bus, self.get_flag(Z_FLAG)) }
            0xD0 => { self.ret(bus, !self.get_flag(C_FLAG)) }
            0xD8 => { self.ret(bus, self.get_flag(C_FLAG)) }
            0xD9 => { self.regs.pc = self.pop(bus); self.ime = true; 4 } // RETI

            // RST (Restart Vectors)
            0xC7 => { self.rst(bus, 0x00); 4 }
            0xCF => { self.rst(bus, 0x08); 4 }
            0xD7 => { self.rst(bus, 0x10); 4 }
            0xDF => { self.rst(bus, 0x18); 4 }
            0xE7 => { self.rst(bus, 0x20); 4 }
            0xEF => { self.rst(bus, 0x28); 4 }
            0xF7 => { self.rst(bus, 0x30); 4 }
            0xFF => { self.rst(bus, 0x38); 4 }

            // --- Rotaciones de Acumulador (Legacy 8080) ---
            0x07 => { // RLCA
                let val = self.regs.a;
                let carry = (val & 0x80) >> 7;
                self.regs.a = (val << 1) | carry;
                self.regs.f = 0; // Z es siempre 0 en RLCA hardware original
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
            0x17 => { // RLA (Rotate Left through Carry)
                let val = self.regs.a;
                let old_c = if self.get_flag(C_FLAG) { 1 } else { 0 };
                let new_c = (val & 0x80) >> 7;
                self.regs.a = (val << 1) | old_c;
                self.regs.f = 0;
                if new_c != 0 { self.regs.f |= C_FLAG; }
                1
            }
            0x1F => { // RRA (Rotate Right through Carry)
                let val = self.regs.a;
                let old_c = if self.get_flag(C_FLAG) { 1 } else { 0 };
                let new_c = val & 0x01;
                self.regs.a = (val >> 1) | (old_c << 7);
                self.regs.f = 0;
                if new_c != 0 { self.regs.f |= C_FLAG; }
                1
            }

            // --- Misc ---
            0x27 => { self.daa(); 1 } // Decimal Adjust
            0x2F => { self.regs.a = !self.regs.a; self.set_flag(N_FLAG, true); self.set_flag(H_FLAG, true); 1 } // CPL
            0x37 => { self.set_flag(N_FLAG, false); self.set_flag(H_FLAG, false); self.set_flag(C_FLAG, true); 1 } // SCF
            0x3F => { // CCF
                self.set_flag(N_FLAG, false); 
                self.set_flag(H_FLAG, false); 
                let c = self.get_flag(C_FLAG); 
                self.set_flag(C_FLAG, !c); 
                1 
            }
            0xF3 => { self.ime = false; 1 } // DI: Disable Interrupts
            0xFB => { self.ime = true; 1 }  // EI: Enable Interrupts (Delayed 1 instr en real hw, aquí directo)

            // --- High RAM I/O ---
            0xE0 => { let off = self.fetch(bus) as u16; bus.write(0xFF00 + off, self.regs.a); 3 } // LDH (n), A
            0xF0 => { let off = self.fetch(bus) as u16; self.regs.a = bus.read(0xFF00 + off); 3 } // LDH A, (n)
            0xE2 => { bus.write(0xFF00 + (self.regs.c as u16), self.regs.a); 2 } // LD (C), A
            0xF2 => { self.regs.a = bus.read(0xFF00 + (self.regs.c as u16)); 2 } // LD A, (C)
            0xEA => { let addr = self.fetch_u16(bus); bus.write(addr, self.regs.a); 4 } // LD (nn), A
            0xFA => { let addr = self.fetch_u16(bus); self.regs.a = bus.read(addr); 4 } // LD A, (nn)
            
            0xE8 => { // ADD SP, e8
                let offset = self.fetch(bus) as i8 as u16;
                let sp = self.regs.sp;
                let res = sp.wrapping_add(offset);
                self.regs.f = 0;
                if (sp & 0xF) + (offset & 0xF) > 0xF { self.regs.f |= H_FLAG; }
                if (sp & 0xFF) + (offset & 0xFF) > 0xFF { self.regs.f |= C_FLAG; }
                self.regs.sp = res;
                4
            }

            // --- PREFIX CB (Extensiones de Bitwise) ---
            0xCB => { self.execute_cb(bus) }
            
            _ => { 
                // Unhandled Opcode
                // println!("Opcode Desconocido: {:#02X}", opcode);
                1 
            }
        }
    }

    /// Ejecuta instrucciones CB: Rotaciones extendidas, Shifts, Bits.
    fn execute_cb(&mut self, bus: &mut Bus) -> u32 {
        let opcode = self.fetch(bus);
        let reg_idx = opcode & 0x07; // Últimos 3 bits dicen el registro
        let mut val = self.get_reg_from_code(reg_idx, bus);
        let cycles = if reg_idx == 6 { 4 } else { 2 }; // (HL) tarda más

        match opcode {
            // RLC r
            0x00..=0x07 => { 
                let carry = (val & 0x80) >> 7;
                val = (val << 1) | carry;
                self.regs.f = 0; 
                if val == 0 { self.regs.f |= Z_FLAG; } 
                if carry != 0 { self.regs.f |= C_FLAG; }
            }
            // RRC r
            0x08..=0x0F => {
                let carry = val & 0x01;
                val = (val >> 1) | (carry << 7);
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if carry != 0 { self.regs.f |= C_FLAG; }
            }
            // RL r
            0x10..=0x17 => {
                let old_c = if self.get_flag(C_FLAG) { 1 } else { 0 };
                let new_c = (val & 0x80) >> 7;
                val = (val << 1) | old_c;
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if new_c != 0 { self.regs.f |= C_FLAG; }
            }
            // RR r
            0x18..=0x1F => {
                let old_c = if self.get_flag(C_FLAG) { 1 } else { 0 };
                let new_c = val & 0x01;
                val = (val >> 1) | (old_c << 7);
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if new_c != 0 { self.regs.f |= C_FLAG; }
            }
            // SLA r (Shift Left Arithmetic)
            0x20..=0x27 => {
                let c = (val & 0x80) >> 7;
                val <<= 1;
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if c != 0 { self.regs.f |= C_FLAG; }
            }
            // SRA r (Shift Right Arithmetic - Keep sign)
            0x28..=0x2F => {
                let c = val & 0x01;
                val = (val as i8 >> 1) as u8;
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if c != 0 { self.regs.f |= C_FLAG; }
            }
            // SWAP r
            0x30..=0x37 => {
                val = (val << 4) | (val >> 4);
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; }
            }
            // SRL r (Shift Right Logical - Zero fill)
            0x38..=0x3F => {
                let c = val & 0x01;
                val >>= 1;
                self.regs.f = 0; if val == 0 { self.regs.f |= Z_FLAG; } if c != 0 { self.regs.f |= C_FLAG; }
            }
            // BIT b, r (Solo actualiza flags, no escribe val)
            0x40..=0x7F => {
                let bit = (opcode >> 3) & 0x07;
                let zero = (val & (1 << bit)) == 0;
                self.set_flag(Z_FLAG, zero);
                self.set_flag(N_FLAG, false);
                self.set_flag(H_FLAG, true);
                return if reg_idx == 6 { 3 } else { 2 }; // Early return, no write back
            }
            // RES b, r (Reset bit)
            0x80..=0xBF => {
                val &= !(1 << ((opcode >> 3) & 0x07));
            }
            // SET b, r (Set bit)
            0xC0..=0xFF => {
                val |= 1 << ((opcode >> 3) & 0x07);
            }
        }

        self.write_reg_cb(bus, reg_idx, val);
        cycles
    }

    // --- ALU / UTILIDADES --- en Rust usamos métodos privados (fn sin pub)

    fn add(&mut self, val: u8) {
        let (res, carry) = self.regs.a.overflowing_add(val);
        // Half carry check: (a & 0xF) + (b & 0xF) > 0xF
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
        self.regs.f = N_FLAG; // Resta -> N=1
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
        // Compare is essentially SUB but discard result
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
        self.set_flag(H_FLAG, (val & 0xF) == 0xF); // Half carry si pasamos de F a 0
        res
    }

    fn dec(&mut self, val: u8) -> u8 {
        let res = val.wrapping_sub(1);
        self.set_flag(Z_FLAG, res == 0);
        self.set_flag(N_FLAG, true);
        self.set_flag(H_FLAG, (val & 0xF) == 0); // Half borrow si pasamos de 0 a F
        res
    }

    fn add_hl(&mut self, val: u16) {
        let hl = self.regs.get_hl();
        let (res, carry) = hl.overflowing_add(val);
        // Half carry en 16 bits (bit 11->12)
        let half = (hl & 0xFFF) + (val & 0xFFF) > 0xFFF;
        self.regs.set_hl(res);
        self.set_flag(N_FLAG, false);
        self.set_flag(H_FLAG, half);
        self.set_flag(C_FLAG, carry);
    }

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

    // --- MEMORY FETCH ---

    fn fetch(&mut self, bus: &mut Bus) -> u8 {
        let v = bus.read(self.regs.pc);
        self.regs.pc = self.regs.pc.wrapping_add(1);
        v
    }

    fn fetch_u16(&mut self, bus: &mut Bus) -> u16 {
        let l = self.fetch(bus) as u16;
        let h = self.fetch(bus) as u16;
        (h << 8) | l // Endianness: Little Endian (Low byte first)
    }

    fn push(&mut self, bus: &mut Bus, val: u16) {
        // Stack crece hacia abajo (direcciones menores)
        self.regs.sp = self.regs.sp.wrapping_sub(1); bus.write(self.regs.sp, (val >> 8) as u8);
        self.regs.sp = self.regs.sp.wrapping_sub(1); bus.write(self.regs.sp, val as u8);
    }

    fn pop(&mut self, bus: &mut Bus) -> u16 {
        // Stack decrece hacia arriba
        let l = bus.read(self.regs.sp) as u16; self.regs.sp = self.regs.sp.wrapping_add(1);
        let h = bus.read(self.regs.sp) as u16; self.regs.sp = self.regs.sp.wrapping_add(1);
        (h << 8) | l
    }

    // --- CONTROL DE FLUJO ---

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

    // --- HELPERS ---

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