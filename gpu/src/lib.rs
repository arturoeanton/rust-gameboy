// gpu/src/lib.rs

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

#[derive(Copy, Clone, PartialEq)]
pub enum Mode {
    HBlank = 0,
    VBlank = 1,
    OamSearch = 2,
    PixelTransfer = 3,
}

pub struct Gpu {
    pub vram: [u8; 0x2000], // 8KB Video RAM
    pub oam: [u8; 0xA0],    // Object Attribute Memory (Sprites)
    pub frame_buffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4],

    // Registros LCD
    pub lcdc: u8, // LCD Control (FF40)
    pub stat: u8, // LCD Status  (FF41)
    pub scy: u8,  // Scroll Y    (FF42)
    pub scx: u8,  // Scroll X    (FF43)
    pub ly: u8,   // LCD Y Line  (FF44)
    pub lyc: u8,  // LY Compare  (FF45)
    pub bgp: u8,  // BG Palette  (FF47) - Importante para Tetris
    pub obp0: u8, // Obj Palette 0 (FF48)
    pub obp1: u8, // Obj Palette 1 (FF49)
    pub wy: u8,   // Window Y    (FF4A)
    pub wx: u8,   // Window X    (FF4B)

    cycles: u32,
}

impl Gpu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            frame_buffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            lcdc: 0x91, 
            stat: 0,
            scy: 0, scx: 0, ly: 0, lyc: 0,
            bgp: 0xFC, obp0: 0xFF, obp1: 0xFF,
            wy: 0, wx: 0,
            cycles: 0,
        }
    }

    pub fn read_vram(&self, addr: u16) -> u8 {
        self.vram[addr as usize]
    }

    pub fn write_vram(&mut self, addr: u16, val: u8) {
        self.vram[addr as usize] = val;
    }

    pub fn step(&mut self, cycles: u32) -> bool {
        // Si el LCD está apagado (Bit 7 de LCDC), no hacemos nada
        if (self.lcdc & 0x80) == 0 {
            self.ly = 0;
            self.stat &= 0xFC; // Modo 0
            self.cycles = 0;
            return false;
        }

        self.cycles += cycles;
        let mut frame_ready = false;

        match self.get_mode() {
            Mode::OamSearch => {
                if self.cycles >= 80 {
                    self.cycles -= 80;
                    self.set_mode(Mode::PixelTransfer);
                }
            }
            Mode::PixelTransfer => {
                if self.cycles >= 172 {
                    self.cycles -= 172;
                    self.set_mode(Mode::HBlank);
                    
                    // ¡AQUÍ DIBUJAMOS LA LÍNEA REAL!
                    self.render_scanline(); 
                }
            }
            Mode::HBlank => {
                if self.cycles >= 204 {
                    self.cycles -= 204;
                    self.ly += 1;

                    if self.ly == 144 {
                        self.set_mode(Mode::VBlank);
                        frame_ready = true;
                    } else {
                        self.set_mode(Mode::OamSearch);
                    }
                }
            }
            Mode::VBlank => {
                if self.cycles >= 456 {
                    self.cycles -= 456;
                    self.ly += 1;

                    if self.ly > 153 {
                        self.ly = 0;
                        self.set_mode(Mode::OamSearch);
                    }
                }
            }
        }
        frame_ready
    }

    // --- RENDERIZADO REAL (Adiós Gradiente) ---

    // gpu/src/lib.rs

    fn render_window(&mut self) {
        // WY es la posición Y donde empieza la ventana
        let wy = self.wy;
        
        // Si la línea actual (LY) es menor que WY, no hay ventana aquí.
        if self.ly < wy { return; }

        // WX es la posición X + 7.
        let wx = self.wx;
        let window_x_pos = wx.wrapping_sub(7);

        // Mapa de Tiles de la Ventana: Bit 6 LCDC (0=9800, 1=9C00)
        let map_base: u16 = if (self.lcdc & 0x40) != 0 { 0x1C00 } else { 0x1800 };

        // Tipo de datos (Signed/Unsigned): Bit 4 LCDC
        let use_unsigned_mode = (self.lcdc & 0x10) != 0;

        // Calcular qué fila de la ventana estamos dibujando
        let window_line = self.ly.wrapping_sub(wy);
        let tile_row = (window_line % 8) as u16;
        let tile_line_idx = (window_line / 8) as u16;

        for x in 0..SCREEN_WIDTH {
            // Si estamos a la izquierda del inicio de la ventana, saltamos
            if (x as u8) < window_x_pos { continue; }

            // Coordenada X relativa dentro de la ventana
            let window_rel_x = (x as u8).wrapping_sub(window_x_pos);

            let tile_col = (window_rel_x / 8) as u16;
            let tile_map_addr = map_base + (tile_line_idx * 32) + tile_col;
            let tile_id = self.vram[tile_map_addr as usize];

            // Calcular dirección de datos (Misma lógica corregida que el BG)
            let tile_data_addr = if use_unsigned_mode {
                (tile_id as u16) * 16
            } else {
                let signed_id = (tile_id as i8) as i16;
                (0x1000 + (signed_id * 16)) as u16
            };

            let byte1 = self.vram[(tile_data_addr + (tile_row * 2)) as usize];
            let byte2 = self.vram[(tile_data_addr + (tile_row * 2) + 1) as usize];

            let bit_idx = 7 - (window_rel_x % 8);
            let lo = (byte1 >> bit_idx) & 1;
            let hi = (byte2 >> bit_idx) & 1;
            let color_id = (hi << 1) | lo;

            let color = self.get_color(color_id, self.bgp);
            
            // La ventana siempre tapa al fondo
            self.set_pixel(x, self.ly as usize, color);
        }
    }
    fn render_scanline(&mut self) {
        // 1. Dibujar Fondo (Background)
        if (self.lcdc & 0x01) != 0 {
            self.render_background();
        }

        // 2. Dibujar Ventana (Window) - ¡NUEVO!
        // Bit 5 LCDC: Habilita la ventana
        if (self.lcdc & 0x20) != 0 {
            self.render_window();
        }

        // 3. Dibujar Sprites
        if (self.lcdc & 0x02) != 0 {
            self.render_sprites();
        }
    }

    fn render_background(&mut self) {
        let y = self.ly;
        
        // 1. Mapa de Tiles: 9800 (offset 1800) o 9C00 (offset 1C00)
        let map_base: u16 = if (self.lcdc & 0x08) != 0 { 0x1C00 } else { 0x1800 };

        // 2. Modo de Datos: ¿Usamos 8000 (unsigned) o 8800 (signed)?
        let use_unsigned_mode = (self.lcdc & 0x10) != 0;

        let map_y = y.wrapping_add(self.scy);
        let tile_row = (map_y % 8) as u16;

        for x in 0..SCREEN_WIDTH {
            let map_x = (x as u8).wrapping_add(self.scx);
            
            let tile_col = (map_x / 8) as u16;
            let tile_line_idx = (map_y / 8) as u16;

            let tile_map_addr = map_base + (tile_line_idx * 32) + tile_col;
            let tile_id = self.vram[tile_map_addr as usize];

            // --- AQUÍ ESTÁ EL FIX ---
            let tile_data_addr = if use_unsigned_mode {
                // Modo 8000: Simple, empieza en 0
                (tile_id as u16) * 16
            } else {
                // Modo 8800 (Signed):
                // El ID 0 está en 0x1000 (GB 0x9000).
                // El ID -128 está en 0x0800 (GB 0x8800).
                let signed_id = (tile_id as i8) as i16;
                
                // Usamos 0x1000 como base central. Rust maneja el +/- del signed_id
                (0x1000 + (signed_id * 16)) as u16
            };
            // ------------------------

            let byte1 = self.vram[(tile_data_addr + (tile_row * 2)) as usize];
            let byte2 = self.vram[(tile_data_addr + (tile_row * 2) + 1) as usize];

            let bit_idx = 7 - (map_x % 8);
            let lo = (byte1 >> bit_idx) & 1;
            let hi = (byte2 >> bit_idx) & 1;
            let color_id = (hi << 1) | lo;

            let color = self.get_color(color_id, self.bgp);
            self.set_pixel(x, y as usize, color);
        }
    }
   
    fn render_sprites(&mut self) {
        // Altura de sprites (8x8 o 8x16) - Bit 2 LCDC
        let sprite_height = if (self.lcdc & 0x04) != 0 { 16 } else { 8 };

        // Recorrer los 40 sprites (OAM son 160 bytes, 4 bytes por sprite)
        // Nota: En hardware real hay límite de 10 sprites por línea. Aquí simplificamos.
        for i in 0..40 {
            let offset = i * 4;
            let sprite_y = self.oam[offset] as i16 - 16;
            let sprite_x = self.oam[offset+1] as i16 - 8;
            let tile_idx = self.oam[offset+2];
            let flags = self.oam[offset+3];

            // Si el sprite no está en la línea actual, saltar
            if (self.ly as i16) < sprite_y || (self.ly as i16) >= (sprite_y + sprite_height) {
                continue;
            }

            let y_flip = (flags & 0x40) != 0;
            let x_flip = (flags & 0x20) != 0;
            let palette = if (flags & 0x10) != 0 { self.obp1 } else { self.obp0 };

            // Calcular qué línea del sprite dibujar
            let mut line = (self.ly as i16 - sprite_y) as u16;
            if y_flip { line = (sprite_height as u16) - 1 - line; }

            // Dirección del tile del sprite
            let tile_addr = (tile_idx as u16 * 16) + (line * 2);
            let byte1 = self.vram[tile_addr as usize];
            let byte2 = self.vram[(tile_addr + 1) as usize];

            for x in 0..8 {
                let pixel_x = sprite_x + x;
                if pixel_x < 0 || pixel_x >= SCREEN_WIDTH as i16 { continue; }

                let bit_idx = if x_flip { x } else { 7 - x };
                let lo = (byte1 >> bit_idx) & 1;
                let hi = (byte2 >> bit_idx) & 1;
                let color_id = (hi << 1) | lo;

                // Sprites: Color 0 es transparente
                if color_id == 0 { continue; }

                let color = self.get_color(color_id, palette);
                
                // Prioridad BG vs OBJ (Bit 7 Flags)
                // Si bit 7 es 1, el sprite va DETRÁS de colores 1-3 del fondo.
                // (Para hacerlo simple ahora, dibujamos encima siempre, Tetris no usa prioridad compleja)
                self.set_pixel(pixel_x as usize, self.ly as usize, color);
            }
        }
    }

    // Traduce Color ID (0-3) a RGBA usando la paleta
    fn get_color(&self, color_id: u8, palette: u8) -> [u8; 4] {
        // Extraer los 2 bits de la paleta correspondientes al ID
        let shade = (palette >> (color_id * 2)) & 0x03;
        
        match shade {
            0 => [0x9B, 0xBC, 0x0F, 0xFF], // Blanco (Verde claro gameboy)
            1 => [0x8B, 0xAC, 0x0F, 0xFF], // Gris claro
            2 => [0x30, 0x62, 0x30, 0xFF], // Gris oscuro
            3 => [0x0F, 0x38, 0x0F, 0xFF], // Negro (Verde oscuro)
            _ => [0, 0, 0, 0],
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        let offset = (y * SCREEN_WIDTH + x) * 4;
        self.frame_buffer[offset] = color[0];
        self.frame_buffer[offset+1] = color[1];
        self.frame_buffer[offset+2] = color[2];
        self.frame_buffer[offset+3] = color[3];
    }

    // Helpers de Estado
    fn get_mode(&self) -> Mode {
        match self.stat & 0x03 {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamSearch,
            3 => Mode::PixelTransfer,
            _ => unreachable!(),
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        self.stat = (self.stat & !0x03) | (mode as u8);
    }
}