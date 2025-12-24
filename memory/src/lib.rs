// memory/src/lib.rs
use mbc::Mbc;
use gpu::Gpu;
use joypad::Joypad;

pub struct Bus {
    pub cartridge: Box<dyn Mbc>,
    pub wram: [u8; 0x2000],
    pub hram: [u8; 0x7F],
    pub gpu: Gpu,
    pub joypad: Joypad,
    
    // Interrupciones
    pub interrupt_enable: u8,
    pub interrupt_flag: u8,

    // --- TIMER SYSTEM ---
    pub div: u16,   // Divider Register (16 bits internamente, solo se ve el alto)
    pub tima: u8,   // Timer Counter
    pub tma: u8,    // Timer Modulo
    pub tac: u8,    // Timer Control
    // --------------------
}

impl Bus {
    pub fn new(cartridge: Box<dyn Mbc>) -> Self {
        Self {
            cartridge,
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            gpu: Gpu::new(),
            joypad: Joypad::new(),
            interrupt_enable: 0,
            interrupt_flag: 0,
            
            // Inicialización del Timer
            div: 0xABCC, // Valor típico al inicio
            tima: 0,
            tma: 0,
            tac: 0,
        }
    }

    // Esta función avanza el reloj del Timer. Se debe llamar desde el bucle principal.
    pub fn tick_timer(&mut self, cycles: u32) {
        // 1. DIV siempre incrementa (16384 Hz)
        self.div = self.div.wrapping_add(cycles as u16 * 4); // x4 porque cycles son M-Cycles

        // 2. TIMA solo incrementa si TAC bit 2 está encendido
        if (self.tac & 0x04) != 0 {
            // Frecuencia depende de bits 0-1 de TAC
            let freq_bit = match self.tac & 0x03 {
                0 => 9,  // 4096 Hz (Bit 9 de system clock)
                1 => 3,  // 262144 Hz (Bit 3)
                2 => 5,  // 65536 Hz (Bit 5)
                3 => 7,  // 16384 Hz (Bit 7)
                _ => unreachable!(),
            };

            // Detectar flanco de subida para incrementar TIMA
            // (Simplificado: incrementamos basado en ciclos acumulados)
            // En una emulación perfecta esto es más complejo, pero esto sirve para el 99% de juegos.
            let mask = 1 << freq_bit;
            // Hack rápido: Usamos el DIV interno como reloj maestro
            // Si el DIV cruza el umbral de la frecuencia, incrementamos TIMA.
            // Para ser más precisos en Rust sin complicarnos:
            // Simplemente incrementaremos TIMA "a ojo" cada X ciclos si está activo.
            // Pero la forma robusta simple es detectar el bit del DIV:
            // (Esta implementación es un compromiso funcional para Tetris/Mario/T&J)
        }
        
        // IMPLEMENTACIÓN ROBUSTA SIMPLIFICADA DEL TIMER
        // Usamos un contador interno implícito en self.div para manejar TIMA
        // Detectamos "Falling Edge" del bit seleccionado del DIV.
        let bit = self.tac & 0x03;
        // ... (Lógica compleja omitida, usamos la versión funcional directa abajo)
    }
    
    // Versión funcional directa del Timer Step
    pub fn step_timer(&mut self, cycles: u32) {
        // DIV aumenta siempre
        let old_div = self.div;
        self.div = self.div.wrapping_add(cycles as u16 * 4);
        
        // Si el Timer está habilitado (TAC bit 2)
        if (self.tac & 0x04) != 0 {
            let freq = match self.tac & 0x03 {
                0 => 1024, // Cada 1024 T-Cycles
                1 => 16,   // Cada 16
                2 => 64,   // Cada 64
                3 => 256,  // Cada 256
                _ => 0,
            };

            // Verificar cuántas veces hay que incrementar TIMA
            // (Usamos un acumulador simple basado en DIV para sincronizar)
            // Forma sencilla: usar un contador separado, pero DIV sirve.
            let counter_bit = match self.tac & 0x03 {
                0 => 9, 1 => 3, 2 => 5, 3 => 7, _=>0
            };
            
            // Detección de flanco de bajada (Falling Edge) para incrementar TIMA
            let old_bit = (old_div >> counter_bit) & 1;
            let new_bit = (self.div >> counter_bit) & 1;
            
            if old_bit == 1 && new_bit == 0 {
                let (new_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = new_tima;
                
                if overflow {
                    self.tima = self.tma; // Reload
                    self.interrupt_flag |= 0x04; // Solicitar Interrupción Timer
                }
            }
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cartridge.read(addr),
            0x8000..=0x9FFF => self.gpu.read_vram(addr - 0x8000),
            0xA000..=0xBFFF => self.cartridge.read(addr),
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            // --- ¡FIX PARA TOM & JERRY! ---
            // Echo RAM: Es un espejo de la WRAM (0xC000). 
            // Si el juego lee E000, le damos lo que hay en C000.
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],
            0xE000..=0xFDFF => 0xFF,
            0xFE00..=0xFE9F => self.gpu.oam[(addr - 0xFE00) as usize],

            // IO Registers
            0xFF00 => self.joypad.read(),
            
            // TIMER READ
            0xFF04 => (self.div >> 8) as u8, // DIV es solo el byte alto
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac,
            
           // 0xFF0F => self.interrupt_flag | (if self.joypad.interrupt_request { 0x10 } else { 0 }),
            // 0xFF0F: Registro de Interrupciones
            // Solo combinamos Joypad. La GPU ya escribió su bit en 'interrupt_flag' desde 'display'.
            0xFF0F => {
                self.interrupt_flag | (if self.joypad.interrupt_request { 0x10 } else { 0 })
            },
            0xFF40..=0xFF4B => self.read_gpu_register(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.interrupt_enable,
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.cartridge.write(addr, val),
            0x8000..=0x9FFF => self.gpu.write_vram(addr - 0x8000, val),
            0xA000..=0xBFFF => self.cartridge.write(addr, val),
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xFE00..=0xFE9F => self.gpu.oam[(addr - 0xFE00) as usize] = val,
            
            0xFF00 => self.joypad.write(val),
            
            // TIMER WRITE
            0xFF04 => self.div = 0, // Escribir en DIV lo resetea a 0
            0xFF05 => self.tima = val,
            0xFF06 => self.tma = val,
            0xFF07 => self.tac = val,
            
            0xFF0F => self.interrupt_flag = val,
            0xFF46 => self.perform_dma(val),
            0xFF40..=0xFF4B => self.write_gpu_register(addr, val),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF => self.interrupt_enable = val,
            _ => {}
        }
    }

    fn perform_dma(&mut self, source_high: u8) {
        let base_addr = (source_high as u16) << 8;
        for i in 0..0xA0 {
            let addr = base_addr + i;
            let byte = self.read(addr); // Usamos read() para simplificar
            self.gpu.oam[i as usize] = byte;
        }
    }

    fn read_gpu_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.gpu.lcdc,
            0xFF41 => self.gpu.stat,
            0xFF42 => self.gpu.scy,
            0xFF43 => self.gpu.scx,
            0xFF44 => self.gpu.ly,
            0xFF45 => self.gpu.lyc,
            0xFF47 => self.gpu.bgp,
            0xFF48 => self.gpu.obp0,
            0xFF49 => self.gpu.obp1,
            0xFF4A => self.gpu.wy,
            0xFF4B => self.gpu.wx,
            _ => 0xFF,
        }
    }

    fn write_gpu_register(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF40 => self.gpu.lcdc = val,
            0xFF41 => self.gpu.stat = val,
            0xFF42 => self.gpu.scy = val,
            0xFF43 => self.gpu.scx = val,
            0xFF45 => self.gpu.lyc = val,
            0xFF47 => self.gpu.bgp = val,
            0xFF48 => self.gpu.obp0 = val,
            0xFF49 => self.gpu.obp1 = val,
            0xFF4A => self.gpu.wy = val,
            0xFF4B => self.gpu.wx = val,
            _ => {}
        }
    }
}