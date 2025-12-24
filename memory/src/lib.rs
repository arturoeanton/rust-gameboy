// memory/src/lib.rs

// Importamos los componentes que se conectan al Bus
use mbc::Mbc;
use gpu::Gpu;
use joypad::Joypad;

/// El Bus de Memoria es el "sistema nervioso" del Game Boy.
/// Conecta la CPU con todos los periféricos mapeando direcciones de memoria (0x0000 - 0xFFFF).
pub struct Bus {
    // El cartucho (ROM + RAM externa + Mapper/MBC)
    pub cartridge: Box<dyn Mbc>,

    // Working RAM (WRAM): 8KB de memoria de propósito general (0xC000 - 0xDFFF)
    pub wram: [u8; 0x2000],

    // High RAM (HRAM): 127 bytes de memoria ultra rápida usada por la CPU (0xFF80 - 0xFFFE)
    pub hram: [u8; 0x7F],

    // Unidad de Procesamiento de Gráficos (Video RAM + OAM + Registros)
    pub gpu: Gpu,

    // Controlador del Joypad (Entrada de botones)
    pub joypad: Joypad,
    
    // --- GESTIÓN DE INTERRUPCIONES ---
    // Interrupt Enable (IE - 0xFFFF): Máscara que dice qué interrupciones permite el juego.
    pub interrupt_enable: u8,  
    
    // Interrupt Flag (IF - 0xFF0F): Banderas que indican qué interrupciones están pendientes.
    // Bit 0: V-Blank, Bit 1: LCD Stat, Bit 2: Timer, Bit 3: Serial, Bit 4: Joypad
    pub interrupt_flag: u8,    
}

impl Bus {
    /// Constructor del Bus: Ensambla todos los componentes.
    pub fn new(cartridge: Box<dyn Mbc>) -> Self {
        Self {
            cartridge,
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            gpu: Gpu::new(),
            joypad: Joypad::new(),
            interrupt_enable: 0,
            interrupt_flag: 0,
        }
    }

    /// Lectura de memoria (La CPU pide un byte en 'addr')
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // 0x0000 - 0x7FFF: ROM del Cartucho (Manejado por MBC)
            0x0000..=0x7FFF => self.cartridge.read(addr),

            // 0x8000 - 0x9FFF: Video RAM (VRAM)
            0x8000..=0x9FFF => self.gpu.read_vram(addr - 0x8000),

            // 0xA000 - 0xBFFF: RAM Externa del Cartucho (Si existe)
            0xA000..=0xBFFF => self.cartridge.read(addr),

            // 0xC000 - 0xDFFF: Working RAM (Memoria interna)
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],

            // 0xE000 - 0xFDFF: Echo RAM (Espejo de WRAM). 
            // Nintendo prohibía usarla, pero por robustez devolvemos 0 o WRAM.
            0xE000..=0xFDFF => 0xFF, 

            // 0xFE00 - 0xFE9F: OAM (Memoria de Sprites/Objetos)
            // Delega a la GPU (o accedemos directo a su array oam si es público)
            0xFE00..=0xFE9F => self.gpu.oam[(addr - 0xFE00) as usize],

            // --- REGISTROS DE HARDWARE (I/O) ---
            
            // 0xFF00: Input del Joypad
            0xFF00 => self.joypad.read(),

            // 0xFF0F: Registro de Banderas de Interrupción (IF)
            // Aquí combinamos la bandera interna con la solicitud del Joypad si existe
            0xFF0F => {
                 self.interrupt_flag | (if self.joypad.interrupt_request { 0x10 } else { 0 })
            },

            // 0xFF40 - 0xFF4B: Registros de Video (LCD Control, Status, Scroll, etc.)
            0xFF40..=0xFF4B => self.read_gpu_register(addr),

            // 0xFF80 - 0xFFFE: High RAM (HRAM)
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],

            // 0xFFFF: Registro de Habilitación de Interrupciones (IE)
            0xFFFF => self.interrupt_enable,

            // Cualquier otra dirección no mapeada devuelve 0xFF
            _ => 0xFF,
        }
    }

    /// Escritura en memoria (La CPU escribe 'val' en 'addr')
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // Cartucho (Puede ser cambio de banco ROM/RAM)
            0x0000..=0x7FFF => self.cartridge.write(addr, val),

            // VRAM
            0x8000..=0x9FFF => self.gpu.write_vram(addr - 0x8000, val),

            // RAM Externa
            0xA000..=0xBFFF => self.cartridge.write(addr, val),

            // WRAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,

            // OAM (Sprites)
            0xFE00..=0xFE9F => self.gpu.oam[(addr - 0xFE00) as usize] = val,

            // Joypad (Selección de fila P14/P15)
            0xFF00 => self.joypad.write(val),

            // Interrupt Flag (La CPU puede limpiar interrupciones escribiendo aquí)
            0xFF0F => self.interrupt_flag = val,

            // --- DMA TRANSFER (Direct Memory Access) ---
            // Cuando se escribe en 0xFF46, se inicia la copia de sprites.
            0xFF46 => self.perform_dma(val),

            // Registros de la GPU
            0xFF40..=0xFF4B => self.write_gpu_register(addr, val),

            // HRAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,

            // Interrupt Enable
            0xFFFF => self.interrupt_enable = val,

            _ => {}
        }
    }

    /// Ejecuta la Transferencia DMA (OAM DMA).
    /// Copia 160 bytes (40 sprites * 4 bytes) desde la ROM o RAM hacia la OAM de la GPU.
    /// 'source_high' es el byte alto de la dirección origen (ej: si es 0xC1, copia desde 0xC100).
    fn perform_dma(&mut self, source_high: u8) {
        let base_addr = (source_high as u16) << 8;

        for i in 0..0xA0 { // 0xA0 = 160 bytes
            let addr = base_addr + i;
            
            // Leemos el byte directamente de los componentes.
            // NOTA: No podemos usar self.read(addr) aquí porque 'self' ya está prestado 
            // como mutable por la función write(), y Rust prohíbe doble préstamo.
            let byte = match addr {
                0x0000..=0x7FFF => self.cartridge.read(addr),
                0x8000..=0x9FFF => self.gpu.vram[(addr - 0x8000) as usize],
                0xA000..=0xBFFF => self.cartridge.read(addr),
                0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
                _ => 0xFF,
            };

            // Escribimos directamente en la memoria de objetos de la GPU
            self.gpu.oam[i as usize] = byte;
        }
    }

    // Helper para leer registros específicos de la GPU mapeados en memoria
   // memory/src/lib.rs

    fn read_gpu_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.gpu.lcdc,
            0xFF41 => self.gpu.stat,
            0xFF42 => self.gpu.scy,
            0xFF43 => self.gpu.scx,
            0xFF44 => self.gpu.ly,
            0xFF45 => self.gpu.lyc,
            
            // --- ESTO ES LO QUE FALTABA ---
            // Sin esto, el juego lee "0" y rompe los colores
            0xFF47 => self.gpu.bgp,
            0xFF48 => self.gpu.obp0,
            0xFF49 => self.gpu.obp1,
            0xFF4A => self.gpu.wy,
            0xFF4B => self.gpu.wx,
            // ------------------------------
            
            _ => 0xFF,
        }
    }
    // Helper para escribir registros específicos de la GPU
    fn write_gpu_register(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF40 => self.gpu.lcdc = val,
            0xFF41 => self.gpu.stat = val,
            0xFF42 => self.gpu.scy = val,
            0xFF43 => self.gpu.scx = val,
            // 0xFF44 (LY) es Read-Only.
            0xFF45 => self.gpu.lyc = val,   // LYC Compare
            
            // --- ¡AQUÍ ESTABA EL PROBLEMA! ---
            // Antes estaban vacíos {}, ahora asignamos el valor.
            0xFF47 => self.gpu.bgp = val,   // Background Palette
            0xFF48 => self.gpu.obp0 = val,  // Object Palette 0
            0xFF49 => self.gpu.obp1 = val,  // Object Palette 1
            // --------------------------------

            0xFF4A => self.gpu.wy = val,    // Window Y
            0xFF4B => self.gpu.wx = val,    // Window X
            _ => {}
        }
    }
}