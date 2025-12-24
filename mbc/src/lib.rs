// mbc/src/lib.rs

/// Interfaz (Trait) común para todos los tipos de cartuchos.
/// Permite al Bus interactuar con el cartucho sin saber si es Tetris (simple) o Pokémon (complejo).
pub trait Mbc {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
}

// =========================================================================
//  TIPO 0: ROM ONLY (Sin Mapper)
//  Usado en juegos pequeños (32KB) como Tetris, Dr. Mario, etc.
// =========================================================================

pub struct RomOnly {
    pub rom: Vec<u8>,
}

impl Mbc for RomOnly {
    fn read(&self, addr: u16) -> u8 {
        // Leemos directamente del array.
        // Usamos .get().unwrap_or para evitar caídas si la CPU lee basura fuera de rango.
        *self.rom.get(addr as usize).unwrap_or(&0xFF)
    }

    fn write(&mut self, _addr: u16, _val: u8) {
        // En un cartucho simple, no hay registros de hardware.
        // Escribir en la ROM no hace nada.
    }
}

// =========================================================================
//  TIPO 1: MBC1 (Memory Bank Controller 1)
//  Usado en Super Mario Land, Zelda Link's Awakening, etc.
//  Permite hasta 2MB de ROM y 32KB de RAM externa.
// =========================================================================

pub struct Mbc1 {
    rom: Vec<u8>,     // Datos del juego
    ram: Vec<u8>,     // Datos de guardado (SRAM)
    rom_bank: u8,     // Banco de ROM seleccionado actualmente
    ram_bank: u8,     // Banco de RAM seleccionado actualmente
    ram_enabled: bool,// "Candado" de seguridad para la RAM
    banking_mode: u8, // Modo 0 (ROM Banking) o Modo 1 (RAM Banking)
}

impl Mbc1 {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            // Inicializamos 32KB de RAM llena de ceros
            ram: vec![0; 0x8000], 
            rom_bank: 1, // Por defecto, el banco conmutable empieza en el 1
            ram_bank: 0,
            ram_enabled: false,
            banking_mode: 0,
        }
    }
}

impl Mbc for Mbc1 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            // ---------------------------------------------------------
            // 0x0000 - 0x3FFF: ROM Banco 0 (Fijo)
            // Siempre contiene el inicio del juego (Header, interrupciones, logo).
            // ---------------------------------------------------------
            0x0000..=0x3FFF => {
                self.rom[addr as usize]
            }
            
            // ---------------------------------------------------------
            // 0x4000 - 0x7FFF: ROM Banco Conmutable (Switchable)
            // Aquí es donde el MBC cambia qué parte del juego ve la CPU.
            // ---------------------------------------------------------
            0x4000..=0x7FFF => {
                let bank = self.rom_bank as usize;
                // Fórmula: (Número de Banco * Tamaño Banco) + Offset dentro del banco
                let offset = (bank * 0x4000) + (addr as usize - 0x4000);
                // El % self.rom.len() es un truco de seguridad:
                // Si el juego pide un banco que no existe, le damos el módulo para no crashear.
                self.rom[offset % self.rom.len()]
            }

            // ---------------------------------------------------------
            // 0xA000 - 0xBFFF: RAM Externa (SRAM)
            // Memoria para guardar partidas (si el cartucho tiene pila).
            // ---------------------------------------------------------
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    let offset = (self.ram_bank as usize * 0x2000) + (addr as usize - 0xA000);
                    self.ram[offset % self.ram.len()]
                } else {
                    0xFF // Si la RAM está bloqueada, el bus devuelve basura (Open Bus = FF)
                }
            }
            
            // Cualquier otra dirección no es responsabilidad del cartucho
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // ---------------------------------------------------------
            // 0x0000 - 0x1FFF: RAM Enable
            // Escribir 0x0A (0000 1010) habilita la RAM. Cualquier otra cosa la bloquea.
            // Esto se hace para evitar corromper los saves al apagar la consola.
            // ---------------------------------------------------------
            0x0000..=0x1FFF => {
                self.ram_enabled = (val & 0x0F) == 0x0A;
            }

            // ---------------------------------------------------------
            // 0x2000 - 0x3FFF: ROM Bank Number (Bits bajos)
            // Selecciona los 5 bits inferiores del banco de ROM.
            // ---------------------------------------------------------
            0x2000..=0x3FFF => {
                let mut bank = val & 0x1F; // Mascara de 5 bits
                if bank == 0 { bank = 1; } // El hardware físico no puede mapear el banco 0 aquí.
                
                // Mantenemos los bits altos (mask 0x60) y cambiamos los bajos.
                self.rom_bank = (self.rom_bank & 0x60) | bank;
            }

            // ---------------------------------------------------------
            // 0x4000 - 0x5FFF: RAM Bank Number o ROM Bank (Bits altos)
            // Este registro hace dos cosas dependiendo del "Banking Mode".
            // ---------------------------------------------------------
            0x4000..=0x5FFF => {
                let bits = val & 0x03; // Solo importan los 2 primeros bits
                if self.banking_mode == 0 {
                    // Modo ROM (Defecto): Los bits se usan para bancos de ROM > 31 (ej. bank 32, 64)
                    self.rom_bank = (self.rom_bank & 0x1F) | (bits << 5);
                } else {
                    // Modo RAM: Selecciona el banco de RAM externa (0-3)
                    self.ram_bank = bits;
                }
            }

            // ---------------------------------------------------------
            // 0x6000 - 0x7FFF: Banking Mode Select
            // 0 = Modo ROM (16MB ROM / 8KB RAM) - Lo más común.
            // 1 = Modo RAM (4MB ROM / 32KB RAM).
            // ---------------------------------------------------------
            0x6000..=0x7FFF => {
                self.banking_mode = val & 0x01;
            }

            // ---------------------------------------------------------
            // 0xA000 - 0xBFFF: Escritura en RAM Externa
            // ---------------------------------------------------------
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    let offset = (self.ram_bank as usize * 0x2000) + (addr as usize - 0xA000);
                    let len = self.ram.len();
                    self.ram[offset % len] = val;
                }
            }

            _ => {}
        }
    }
}

// =========================================================================
//  FACTORY: EL DETECTOR DE CARTUCHOS
//  Esta función DEBE ser pública (pub) para que main.rs pueda usarla.
// =========================================================================

pub fn new_cartridge(data: Vec<u8>) -> Box<dyn Mbc> {
    // La dirección 0x0147 del header contiene el ID del tipo de cartucho
    let cartridge_type = data.get(0x0147).unwrap_or(&0);

    match cartridge_type {
        // ID 0x00: ROM ONLY (Tetris usa este)
        0x00 => Box::new(RomOnly { rom: data }),

        // ID 0x01 a 0x03: MBC1 (Con o sin RAM/Batería)
        0x01 | 0x02 | 0x03 => Box::new(Mbc1::new(data)),

        // Fallback: Si no conocemos el chip, intentamos correrlo como MBC1
        // (Muchos juegos funcionan así aunque no sea exacto)
        _ => {
            println!("Cartucho tipo {:#04X} no implementado oficialmente. Usando MBC1.", cartridge_type);
            Box::new(Mbc1::new(data))
        }
    }
}