// gpu/src/lib.rs

/// Resolución nativa del Game Boy.
/// 'usize' es el tipo preferido para indexación de arrays en Rust.
pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

/// Los estados del ciclo de renderizado (Modos de la PPU).
/// #[derive(...)]: Macros que implementan Traits automáticamente.
/// - Copy/Clone: Permiten copiar el enum por valor (como un int primitivo).
/// - PartialEq: Permite comparar (mode == Mode::HBlank).
#[derive(Copy, Clone, PartialEq)]
pub enum Mode {
    HBlank = 0,        // Periodo de descanso horizontal (al final de una línea).
    VBlank = 1,        // Periodo de descanso vertical (al final de la pantalla).
    OamSearch = 2,     // Buscando sprites que intersectan la línea actual.
    PixelTransfer = 3, // Enviando píxeles al LCD driver.
}

pub struct Gpu {
    // VRAM (Video RAM): 8KB para Tiles y Mapas.
    pub vram: [u8; 0x2000],
    // OAM (Object Attribute Memory): 160 bytes para 40 sprites.
    pub oam: [u8; 0xA0],
    // Buffer de píxeles: Array lineal RGBA. 
    // Equivalente a []byte en Go, pero tamaño fijo en stack/struct.
    pub frame_buffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4],

    // --- REGISTROS DE HARDWARE LCD ---
    pub lcdc: u8, // Control (On/Off, Map selection, etc)
    pub stat: u8, // Status (Mode flag, Interrupt enables)
    pub scy: u8,  // Scroll Y (Fondo)
    pub scx: u8,  // Scroll X (Fondo)
    pub ly: u8,   // Línea actual (0-153)
    pub lyc: u8,  // Línea de comparación (para interrupciones)
    pub bgp: u8,  // Paleta de Fondo
    pub obp0: u8, // Paleta de Objetos 0
    pub obp1: u8, // Paleta de Objetos 1
    pub wy: u8,   // Posición Y de la Ventana
    pub wx: u8,   // Posición X de la Ventana
    
    // Solicitud de interrupción interna hacia el Bus.
    pub request_stat_interrupt: bool,

    // Contador de ciclos para la máquina de estados del PPU.
    cycles: u32,
}

impl Gpu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            frame_buffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            lcdc: 0x91, // LCD encendido por defecto, BG encendido.
            stat: 0,
            scy: 0, scx: 0, 
            ly: 0, lyc: 0,
            bgp: 0xFC, obp0: 0xFF, obp1: 0xFF, // Paletas por defecto
            wy: 0, wx: 0,
            request_stat_interrupt: false,
            cycles: 0,
        }
    }

    // Getters y Setters rápidos inline
    pub fn read_vram(&self, addr: u16) -> u8 { self.vram[addr as usize] }
    pub fn write_vram(&mut self, addr: u16, val: u8) { self.vram[addr as usize] = val; }

    /// Avanza el estado de la GPU.
    /// Retorna 'true' si se completó un frame (VBlank start) para refrescar la UI.
    pub fn step(&mut self, cycles: u32) -> bool {
        self.request_stat_interrupt = false;

        // Si el LCD está apagado (Bit 7 de LCDC), reseteamos estado y salimos.
        if (self.lcdc & 0x80) == 0 {
            self.ly = 0;
            self.stat &= 0xFC; // Forzamos Modo 0 (HBlank) en STAT
            self.cycles = 0;
            return false;
        }

        self.cycles += cycles;
        let mut frame_ready = false;

        // Máquina de Estados del PPU (Mode 2 -> 3 -> 0 ... -> 1)
        match self.get_mode() {
            Mode::OamSearch => {
                // Modo 2: Dura 80 ciclos (búsqueda de sprites)
                if self.cycles >= 80 {
                    self.cycles -= 80;
                    self.set_mode(Mode::PixelTransfer);
                }
            }
            Mode::PixelTransfer => {
                // Modo 3: Dura ~172 ciclos (dibujado)
                if self.cycles >= 172 {
                    self.cycles -= 172;
                    self.set_mode(Mode::HBlank);
                    
                    // Verificación de Interrupción HBlank (STAT Bit 3)
                    if (self.stat & 0x08) != 0 {
                        self.request_stat_interrupt = true;
                    }
                    
                    // Renderizamos la scanline completa al buffer
                    self.render_scanline();
                }
            }
            Mode::HBlank => {
                // Modo 0: Dura ~204 ciclos (descanso horizontal)
                if self.cycles >= 204 {
                    self.cycles -= 204;
                    self.ly += 1; // Avanzamos a la siguiente línea
                    
                    self.check_lyc(); // Chequear coincidencia LY=LYC

                    if self.ly == 144 {
                        // Terminó la pantalla visible -> VBlank
                        self.set_mode(Mode::VBlank);
                        
                        // Interrupción VBlank (STAT Bit 4)
                        if (self.stat & 0x10) != 0 {
                            self.request_stat_interrupt = true;
                        }
                        frame_ready = true; // Avisamos que hay nuevo frame
                    } else {
                        // Nueva línea visible -> OamSearch
                        self.set_mode(Mode::OamSearch);
                        
                        // Interrupción OAM (STAT Bit 5)
                        if (self.stat & 0x20) != 0 {
                            self.request_stat_interrupt = true;
                        }
                    }
                }
            }
            Mode::VBlank => {
                // Modo 1: Dura 4560 ciclos (10 líneas * 456 ciclos)
                if self.cycles >= 456 {
                    self.cycles -= 456;
                    self.ly += 1;
                    self.check_lyc(); 

                    if self.ly > 153 {
                        // Fin de VBlank, volvemos al principio (Línea 0)
                        self.ly = 0;
                        self.set_mode(Mode::OamSearch);
                        if (self.stat & 0x20) != 0 {
                            self.request_stat_interrupt = true;
                        }
                    }
                }
            }
        }
        frame_ready
    }

    /// Compara LY con LYC y actualiza el registro STAT y solicita interrupción si corresponde.
    fn check_lyc(&mut self) {
        if self.ly == self.lyc {
            self.stat |= 0x04; // Bit 2: Coincidence Flag
            if (self.stat & 0x40) != 0 { // Check Bit 6: LYC Stats Interrupt Enable
                self.request_stat_interrupt = true;
            }
        } else {
            self.stat &= !0x04;
        }
    }

    /// Orquesta el renderizado de la línea actual.
    fn render_scanline(&mut self) {
        // En Go esto serían 'if' simples comprobando flags.
        if (self.lcdc & 0x01) != 0 { self.render_background(); } // Bit 0: BG Enable
        if (self.lcdc & 0x20) != 0 { self.render_window(); }     // Bit 5: Windows Enable
        if (self.lcdc & 0x02) != 0 { self.render_sprites(); }    // Bit 1: OBJ Enable
    }

    /// Dibuja la Capa de Fondo (Background).
    fn render_background(&mut self) {
        let y = self.ly; // Copia local (u8 es Copy)
        
        // Selección de mapa de tiles (0x9800 o 0x9C00)
        let map_base: u16 = if (self.lcdc & 0x08) != 0 { 0x1C00 } else { 0x1800 };
        // Selección de modo de datos de tiles (Signed vs Unsigned)
        let use_unsigned = (self.lcdc & 0x10) != 0;
        
        // Posición real en el mapa considerando SCROLL
        let map_y = y.wrapping_add(self.scy);
        
        // Qué fila (0-7) dentro del tile de 8x8 estamos dibujando
        let tile_row = (map_y % 8) as u16; 
        let tile_line_idx = (map_y / 8) as u16; // Índice vertical de tiles (0-31)

        // Iteramos píxel por píxel de la línea (0 a 159)
        for x in 0..SCREEN_WIDTH {
            let map_x = (x as u8).wrapping_add(self.scx);
            let tile_col = (map_x / 8) as u16; // Índice horizontal de tiles
            
            // Leemos el ID del tile desde el mapa
            let tile_map_addr = map_base + (tile_line_idx * 32) + tile_col;
            let tile_id = self.vram[tile_map_addr as usize];
            
            // Calculamos dirección de los datos gráficos
            let tile_data_addr = self.get_tile_data_addr(tile_id, use_unsigned);

            // Los tiles de GB tienen 2 bits por píxel (2bpp), almacenados en 2 bytes planos.
            let byte1 = self.vram[(tile_data_addr + (tile_row * 2)) as usize];
            let byte2 = self.vram[(tile_data_addr + (tile_row * 2) + 1) as usize];
            
            // Extraemos el color bit a bit.
            // El bit 7 es el píxel izquierdo, bit 0 el derecho.
            let bit_idx = 7 - (map_x % 8);
            let lo = (byte1 >> bit_idx) & 1;
            let hi = (byte2 >> bit_idx) & 1;
            let color_id = (hi << 1) | lo; // Combina (hi, lo) -> 0..3

            // Traducimos ID a color real usando paleta
            let color = self.get_color(color_id, self.bgp);
            self.set_pixel(x, y as usize, color);
        }
    }

    fn render_window(&mut self) {
        let wy = self.wy;
        // Si la línea actual está antes de que empiece la ventana, salimos.
        if self.ly < wy { return; }

        let wx = self.wx;
        // WX tiene un offset de +7 por hardware.
        let window_x_pos = wx.wrapping_sub(7);

        let map_base: u16 = if (self.lcdc & 0x40) != 0 { 0x1C00 } else { 0x1800 };
        let use_unsigned = (self.lcdc & 0x10) != 0;
        
        let window_line = self.ly.wrapping_sub(wy);
        let tile_row = (window_line % 8) as u16;
        let tile_line_idx = (window_line / 8) as u16;

        for x in 0..SCREEN_WIDTH {
            if (x as u8) < window_x_pos { continue; }
            
            let window_rel_x = (x as u8).wrapping_sub(window_x_pos);
            let tile_col = (window_rel_x / 8) as u16;
            
            let tile_map_addr = map_base + (tile_line_idx * 32) + tile_col;
            let tile_id = self.vram[tile_map_addr as usize];
            let tile_data_addr = self.get_tile_data_addr(tile_id, use_unsigned);

            let byte1 = self.vram[(tile_data_addr + (tile_row * 2)) as usize];
            let byte2 = self.vram[(tile_data_addr + (tile_row * 2) + 1) as usize];

            let bit_idx = 7 - (window_rel_x % 8);
            let lo = (byte1 >> bit_idx) & 1;
            let hi = (byte2 >> bit_idx) & 1;
            let color = self.get_color((hi << 1) | lo, self.bgp);

            self.set_pixel(x, self.ly as usize, color);
        }
    }

    fn render_sprites(&mut self) {
        // Bit 2: Tamaño de Sprite (8x8 vs 8x16)
        let sprite_height = if (self.lcdc & 0x04) != 0 { 16 } else { 8 };
        
        // Iteramos los 40 sprites posibles.
        // Nota: En un emulador preciso, hay límite de 10 sprites por línea. Aquí simplificamos.
        for i in 0..40 {
            let idx = i * 4;
            let y_pos = self.oam[idx] as i16 - 16;
            let x_pos = self.oam[idx+1] as i16 - 8;
            let tile_idx = self.oam[idx+2];
            let flags = self.oam[idx+3];

            // Verificamos si el sprite intercepta la línea actual ('scanline').
            if (self.ly as i16) < y_pos || (self.ly as i16) >= (y_pos + sprite_height) { continue; }

            // Flags de volteo
            let y_flip = (flags & 0x40) != 0;
            let x_flip = (flags & 0x20) != 0;
            // Palette selection (Non-CGB)
            let palette = if (flags & 0x10) != 0 { self.obp1 } else { self.obp0 };

            // Cálculo de línea interna del sprite
            let mut line = (self.ly as i16 - y_pos) as u16;
            if y_flip { line = sprite_height as u16 - 1 - line; }
            
            // En modo 8x16, el bit menor del tile index se ignora.
            let actual_tile_idx = if sprite_height == 16 { tile_idx & 0xFE } else { tile_idx };
            
            let tile_addr = (actual_tile_idx as u16 * 16) + (line * 2);
            let byte1 = self.vram[tile_addr as usize];
            let byte2 = self.vram[(tile_addr + 1) as usize];

            // Loop de píxeles horizontales (8 píxeles)
            for pixel_x in 0..8 {
                let screen_x = x_pos + pixel_x;
                // Clip sprites que salen de la pantalla
                if screen_x < 0 || screen_x >= SCREEN_WIDTH as i16 { continue; }

                let bit_idx = if x_flip { pixel_x } else { 7 - pixel_x };
                let lo = (byte1 >> bit_idx) & 1;
                let hi = (byte2 >> bit_idx) & 1;
                let color_id = (hi << 1) | lo;

                if color_id == 0 { continue; } // Color 0 en OBJ es transparente

                let color = self.get_color(color_id, palette);
                
                // Prioridad (Bit 7): Si es 1, el sprite se oculta detrás de colores de fondo != 0.
                // Aquí omitimos esa lógica para simplificar, dibujando siempre encima.
                self.set_pixel(screen_x as usize, self.ly as usize, color);
            }
        }
    }

    /// Helper para obtener dirección de datos de tiles.
    /// Maneja la extraña aritmética "Signed" del modo 0x8800.
    fn get_tile_data_addr(&self, tile_id: u8, use_unsigned: bool) -> u16 {
        if use_unsigned { 
            (tile_id as u16) * 16 
        } else { 
            // Truco: Cast a i8 (con signo) e interpretar desde base 0x1000.
            // 0x1000 relativo a VRAM es 0x9000 unallocated space.
            (0x1000 + ((tile_id as i8) as i16 * 16)) as u16 
        }
    }

    /// Traduce un Color ID (0-3) a un color RGBA del array.
    fn get_color(&self, color_id: u8, palette: u8) -> [u8; 4] {
        // La paleta empaqueta 4 colores en un byte (2 bits cada uno).
        match (palette >> (color_id * 2)) & 0x03 {
            0 => [0x9B, 0xBC, 0x0F, 0xFF], // Blanco (Verde claro clásico)
            1 => [0x8B, 0xAC, 0x0F, 0xFF], // Gris claro
            2 => [0x30, 0x62, 0x30, 0xFF], // Gris oscuro
            3 => [0x0F, 0x38, 0x0F, 0xFF], // Negro (Verde oscuro)
            _ => [0, 0, 0, 0],
        }
    }

    /// Escribe un píxel en el framebuffer lineal.
    fn set_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        let offset = (y * SCREEN_WIDTH + x) * 4;
        // copy_from_slice es la forma eficiente de copiar arrays en Rust.
        self.frame_buffer[offset..offset+4].copy_from_slice(&color);
    }
    
    // Helpers para manejo del Enum State
    fn get_mode(&self) -> Mode {
        // Mapeo seguro de u8 a Enum. Si hay un valor inválido, panic (unreachable).
        match self.stat & 0x03 { 0=>Mode::HBlank, 1=>Mode::VBlank, 2=>Mode::OamSearch, 3=>Mode::PixelTransfer, _=>unreachable!() }
    }
    fn set_mode(&mut self, mode: Mode) { 
        self.stat = (self.stat & !0x03) | (mode as u8); 
    }
}