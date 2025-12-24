// memory/src/lib.rs

use mbc::Mbc;
use gpu::Gpu;
use joypad::Joypad;

/// El Bus de Memoria es el "sistema nervioso" del Game Boy.
/// Conecta la CPU con todos los periféricos mapeando direcciones de memoria (0x0000 - 0xFFFF).
///
/// Concepto Rust vs Go:
/// En Rust, preferimos la composición (`struct` con otros `structs`) sobre la herencia.
/// El Bus es dueño (owner) de todos los componentes.
pub struct Bus {
    // El cartucho (ROM + RAM externa + Mapper/MBC)
    // - Box<dyn Mbc>: Puntero al Heap (Box) de un objeto que implementa el Trait Mbc (dyn).
    //   Esto permite polimorfismo en tiempo de ejecución, similar a una Interface en Go.
    pub cartridge: Box<dyn Mbc>,

    // Working RAM (WRAM): 8KB de memoria de propósito general (0xC000 - 0xDFFF).
    // [u8; 0x2000]: Array de tamaño fijo en el Stack (o dentro del struct).
    // Es diferente a un slice (`[]byte` en Go) que es una vista dinámica.
    pub wram: [u8; 0x2000],

    // High RAM (HRAM): 127 bytes de memoria ultra rápida usada por la CPU (0xFF80 - 0xFFFE).
    pub hram: [u8; 0x7F],

    // Unidad de Procesamiento de Gráficos (Video RAM + OAM + Registros).
    pub gpu: Gpu,

    // Controlador del Joypad (Entrada de botones).
    pub joypad: Joypad,
    
    // --- GESTIÓN DE INTERRUPCIONES ---
    // Interrupt Enable (IE - 0xFFFF): Máscara que dice qué interrupciones permite el juego.
    pub interrupt_enable: u8,  
    
    // Interrupt Flag (IF - 0xFF0F): Banderas que indican qué interrupciones están pendientes.
    // Bit 0: V-Blank, Bit 1: LCD Stat, Bit 2: Timer, Bit 3: Serial, Bit 4: Joypad.
    pub interrupt_flag: u8,

    // --- SISTEMA DE TIMER ---
    // El Game Boy tiene un timer interno complejo.
    // DIV: Registro divisor, siempre incrementa (16384Hz).
    pub div: u16,   
    // TIMA: Contador del Timer principal (interrupción al desbordar).
    pub tima: u8,   
    // TMA: Modulo del Timer (valor al que se resetea TIMA).
    pub tma: u8,    
    // TAC: Control del Timer (frecuencia start/stop).
    pub tac: u8,    
}

impl Bus {
    /// Constructor del Bus: Ensambla todos los componentes.
    /// En Rust, es convención usar `new` como constructor, aunque es solo una función estática.
    pub fn new(cartridge: Box<dyn Mbc>) -> Self {
        Self {
            cartridge,
            // Inicialización de arrays con valor repetido [valor; tamaño]
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            gpu: Gpu::new(),
            joypad: Joypad::new(),
            interrupt_enable: 0,
            interrupt_flag: 0,
            
            // Estado inicial del hardware
            div: 0xABCC, // Valor aleatorio típico al arranque
            tima: 0,
            tma: 0,
            tac: 0,
        }
    }
    
    /// Avanza el Timer del sistema. Se llama en cada ciclo de la CPU.
    /// 'cycles' son los ciclos de máquina (M-Cycles) pasados.
    pub fn step_timer(&mut self, cycles: u32) {
        // 1. DIV (Divider Register) siempre incrementa.
        // Es un contador de 16 bits, pero solo el byte alto es visible como registro DIV (0xFF04).
        // wrapping_add: En Rust, el overflow en modo debug causa panic. Usamos wrapping explícito.
        let old_div = self.div;
        self.div = self.div.wrapping_add(cycles as u16 * 4); // x4 porque T-Cycles = 4 * M-Cycles
        
        // 2. TIMA (Timer Counter). Solo cuenta si el Bit 2 de TAC está encendido.
        if (self.tac & 0x04) != 0 {
            // Frecuencia depende de los bits 0-1 de TAC.
            // Mapeamos a qué bit del contador global (sistema) debemos mirar para el cambio.
            let counter_bit = match self.tac & 0x03 {
                0 => 9, // 4096 Hz
                1 => 3, // 262144 Hz
                2 => 5, // 65536 Hz
                3 => 7, // 16384 Hz
                _ => 0,
            };
            
            // Detectamos "Falling Edge" (Flanco de Bajada) del bit seleccionado.
            // Es una peculiaridad del hardware de GB: TIMA incrementa cuando ese bit pasa de 1 a 0.
            let old_bit = (old_div >> counter_bit) & 1;
            let new_bit = (self.div >> counter_bit) & 1;
            
            if old_bit == 1 && new_bit == 0 {
                // Incrementamos TIMA
                let (new_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = new_tima;
                
                // Si TIMA desborda (255 -> 0), se recarga con TMA y se pide interrupción.
                if overflow {
                    self.tima = self.tma; 
                    self.interrupt_flag |= 0x04; // Bit 2: Timer Interrupt
                }
            }
        }
    }

    /// Lectura de memoria (La CPU pide un byte en 'addr').
    /// El 'match' en Rust es como un switch superpoderoso. Puede hacer matching de rangos (..=).
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM del Cartucho
            0x0000..=0x7FFF => self.cartridge.read(addr),

            // Video RAM (VRAM) - Delegado a la GPU
            0x8000..=0x9FFF => self.gpu.read_vram(addr - 0x8000),

            // RAM Externa del Cartucho (Save RAM)
            0xA000..=0xBFFF => self.cartridge.read(addr),

            // Working RAM (Memoria interna 8KB)
            // 'addr - 0xC000' nos da el offset desde 0.
            // 'as usize': Rust requiere usize para indexar arrays/slices. No hay cast implícito.
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],

            // Echo RAM: Espejo de WRAM.
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],

            // OAM (Object Attribute Memory) - Sprites
            0xFE00..=0xFE9F => self.gpu.oam[(addr - 0xFE00) as usize],

            // --- REGISTROS DE HARDWARE (I/O) ---
            
            // Joypad
            0xFF00 => self.joypad.read(),
            
            // Timer Registers
            0xFF04 => (self.div >> 8) as u8, // Solo se lee el byte alto de DIV
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac,
            
            // Registro de Interrupciones (IF)
            // Combinamos las flags internas con la señal del Joypad si está presionado.
            // 'if expr { val } else { val }' es una expresión en Rust (como operador ternario).
            0xFF0F => {
                self.interrupt_flag | (if self.joypad.interrupt_request { 0x10 } else { 0 })
            },

            // Registros de la GPU (LCDC, STAT, SCY, SCX, LY, etc.)
            0xFF40..=0xFF4B => self.read_gpu_register(addr),

            // High RAM (HRAM)
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],

            // Interrupt Enable (IE)
            0xFFFF => self.interrupt_enable,

            // Direcciones no usadas devuelven 0xFF (Bus flotante)
            _ => 0xFF,
        }
    }

    /// Escritura en memoria (La CPU escribe 'val' en 'addr')
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.cartridge.write(addr, val),
            0x8000..=0x9FFF => self.gpu.write_vram(addr - 0x8000, val),
            0xA000..=0xBFFF => self.cartridge.write(addr, val),
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            
            // Echo RAM (usualmente read-only o escribe en WRAM, aquí lo tratamos como read-only write)
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val,

            0xFE00..=0xFE9F => self.gpu.oam[(addr - 0xFE00) as usize] = val,
            
            0xFF00 => self.joypad.write(val),
            
            // Timer Registers
            0xFF04 => self.div = 0, // Escribir cualquier valor resetea DIV a 0
            0xFF05 => self.tima = val,
            0xFF06 => self.tma = val,
            0xFF07 => self.tac = val,
            
            0xFF0F => self.interrupt_flag = val,
            
            // DMA Transfer (Direct Memory Access)
            // Inicia copia rápida de memoria a OAM.
            0xFF46 => self.perform_dma(val),

            0xFF40..=0xFF4B => self.write_gpu_register(addr, val),
            
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF => self.interrupt_enable = val,
            _ => {}
        }
    }

    /// DMA Transfer (Direct Memory Access)
    /// Copia 160 bytes desde addr (xx00) a OAM (FE00).
    /// Es crítico para refrescar sprites rápidamente.
    fn perform_dma(&mut self, source_high: u8) {
        let base_addr = (source_high as u16) << 8;
        for i in 0..0xA0 { // 160 bytes
            let addr = base_addr + i;
            // Usamos self.read() para manejar correctamente si la fuente es ROM, WRAM, etc.
            let byte = self.read(addr);
            self.gpu.oam[i as usize] = byte;
        }
    }

    // Helpers para registros GPU
    fn read_gpu_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.gpu.lcdc, // LCD Control
            0xFF41 => self.gpu.stat, // LCD Status
            0xFF42 => self.gpu.scy,  // Scroll Y
            0xFF43 => self.gpu.scx,  // Scroll X
            0xFF44 => self.gpu.ly,   // LCD Y (Línea actual)
            0xFF45 => self.gpu.lyc,  // LY Compare
            0xFF47 => self.gpu.bgp,  // Palette Background
            0xFF48 => self.gpu.obp0, // Object Palette 0
            0xFF49 => self.gpu.obp1, // Object Palette 1
            0xFF4A => self.gpu.wy,   // Window Y
            0xFF4B => self.gpu.wx,   // Window X
            _ => 0xFF,
        }
    }

    fn write_gpu_register(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF40 => {
                self.gpu.lcdc = val;
                // Opcional: Si apagan el LCD, algunos emuladores resetean LY aquí mismo
                // para ser más precisos, aunque tu gpu.step ya lo maneja.
                if (val & 0x80) == 0 {
                    self.gpu.ly = 0;
                    self.gpu.stat &= 0xFC; // Forzar modo HBlank
                    // self.gpu.cycles = 0; // Resetear reloj interno PPU
                }
            },
            0xFF41 => {
                // FIX CRÍTICO: STAT
                // Los bits 0-2 son Read-Only (Mode flag + Coincidence flag).
                // El juego solo puede tocar los bits 3-6 (Interrupt Enables).
                self.gpu.stat = (self.gpu.stat & 0x07) | (val & 0xF8);
            },
            0xFF42 => self.gpu.scy = val,
            0xFF43 => self.gpu.scx = val,
            
            0xFF44 => {
                // FIX CRÍTICO: LY
                // Es Read-Only. Escribir cualquier valor lo resetea a 0.
                self.gpu.ly = 0; 
            },
            
            0xFF45 => self.gpu.lyc = val,
            0xFF46 => self.perform_dma(val), // Este ya lo tenías bien aparte
            0xFF47 => self.gpu.bgp = val,
            0xFF48 => self.gpu.obp0 = val,
            0xFF49 => self.gpu.obp1 = val,
            0xFF4A => self.gpu.wy = val,
            0xFF4B => self.gpu.wx = val,
            _ => {}
        }
    }
}