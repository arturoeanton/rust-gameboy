// mbc/src/lib.rs

/// Interfaz (Trait) común para todos los tipos de cartuchos.
/// Permite al Bus interactuar con el cartucho sin saber si es Tetris (simple) o Pokémon (complejo).
///
/// Concepto Rust vs Go:
/// - Un `trait` en Rust es casi idéntico a una `interface` en Go.
/// - Define un contrato de comportamiento.
/// - `dyn Mbc` significa "Dynamic dispatch", indicando que el tipo exacto se resolverá en tiempo de ejecución.
pub trait Mbc {
    fn read(&self, addr: u16) -> u8;
    // &mut self indica que la escritura puede cambiar el estado interno del struct (ej: cambiar de banco).
    fn write(&mut self, addr: u16, val: u8);
}

// =========================================================================
//  TIPO 0: ROM ONLY (Sin Mapper)
//  Usado en juegos pequeños (32KB) como Tetris, Dr. Mario, etc.
// =========================================================================

pub struct RomOnly {
    // Vec<u8> es un array dinámico en el Heap (similar a una slice []byte en Go).
    pub rom: Vec<u8>,
}

impl Mbc for RomOnly {
    fn read(&self, addr: u16) -> u8 {
        // Leemos directamente del vector.
        // .get() devuelve Option<&u8>. Usamos unwrap_or para seguridad.
        // * desreferencia el puntero &u8 a u8.
        *self.rom.get(addr as usize).unwrap_or(&0xFF)
    }

    fn write(&mut self, _addr: u16, _val: u8) {
        // En un cartucho simple, no hay registros de hardware.
        // Escribir en la ROM no hace nada.
        // El guion bajo en _addr suprime el warning de "variable no usada".
    }
}

// =========================================================================
//  TIPO 1: MBC1 (Memory Bank Controller 1)
//  Usado en Super Mario Land, Zelda Link's Awakening, etc.
//  Permite hasta 2MB de ROM y 32KB de RAM externa.
// =========================================================================

pub struct Mbc1 {
    rom: Vec<u8>,     
    ram: Vec<u8>,     
    rom_bank: u8,     // Banco de ROM seleccionado actualmente (1-127)
    ram_bank: u8,     // Banco de RAM seleccionado actualmente (0-3)
    ram_enabled: bool,// "Candado" de seguridad para la RAM
    banking_mode: u8, // Modo 0 (ROM Banking) o Modo 1 (RAM Banking)
}

impl Mbc1 {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            // Inicializamos 32KB de RAM llena de ceros.
            // vec! es una macro para crear vectores rápidamente.
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
                // El % self.rom.len() asegura que no leamos fuera del vector si el juego pide un banco fantasma.
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
                    0xFF // Open Bus
                }
            }
            
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // ---------------------------------------------------------
            // 0x0000 - 0x1FFF: RAM Enable
            // Escribir 0x0A habilita la RAM. Cualquier otra cosa la bloquea.
            // ---------------------------------------------------------
            0x0000..=0x1FFF => {
                self.ram_enabled = (val & 0x0F) == 0x0A;
            }

            // ---------------------------------------------------------
            // 0x2000 - 0x3FFF: ROM Bank Number
            // Selecciona los 5 bits inferiores del banco de ROM.
            // ---------------------------------------------------------
            0x2000..=0x3FFF => {
                let mut bank = val & 0x1F; 
                if bank == 0 { bank = 1; } // El banco 0 no se mapea aquí, se convierte en 1.
                
                // Mantenemos los bits altos y cambiamos los bajos.
                self.rom_bank = (self.rom_bank & 0x60) | bank;
            }

            // ---------------------------------------------------------
            // 0x4000 - 0x5FFF: RAM Bank Number / ROM Bank High
            // ---------------------------------------------------------
            0x4000..=0x5FFF => {
                let bits = val & 0x03;
                if self.banking_mode == 0 {
                    // Modo ROM: Son bits altos para el número de banco ROM
                    self.rom_bank = (self.rom_bank & 0x1F) | (bits << 5);
                } else {
                    // Modo RAM: Selecciona banco de RAM
                    self.ram_bank = bits;
                }
            }

            // ---------------------------------------------------------
            // 0x6000 - 0x7FFF: Banking Mode Select
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
//  FACTORY: DETECTOR DE CARTUCHOS
//  En Rust, el polimorfismo de retorno se logra con Box<dyn Trait>.
//  Devolvemos un puntero a "algo que implementa Mbc".
// =========================================================================

pub fn new_cartridge(data: Vec<u8>) -> Box<dyn Mbc> {
    // Leemos el byte 0x147 del header para identificar el tipo.
    let cartridge_type = *data.get(0x0147).unwrap_or(&0);

    match cartridge_type {
        0x00 => Box::new(RomOnly { rom: data }),
        
        // MBC1 es el más común (Mario Land, Tetris, Zelda).
        0x01 | 0x02 | 0x03 => Box::new(Mbc1::new(data)),

        // MBC3 (Pokemon Red/Blue) - Usaremos MBC1 como fallback por ahora.
        // 0x11 | 0x12 | 0x13 => Box::new(Mbc3::new(data)),

        _ => {
            println!("Tipo de cartucho {:#04X} no soportado oficialmente. Usando fallback a MBC1.", cartridge_type);
            Box::new(Mbc1::new(data))
        }
    }
}