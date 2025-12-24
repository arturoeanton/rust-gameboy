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
    pub vram: [u8; 0x2000],
    pub oam: [u8; 0xA0],
    pub frame_buffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4],

    // Registros
    pub lcdc: u8, pub stat: u8, pub scy: u8, pub scx: u8,
    pub ly: u8, pub lyc: u8,
    pub bgp: u8, pub obp0: u8, pub obp1: u8,
    pub wy: u8, pub wx: u8,
    
    // Bandera interna para solicitar interrupción LCD STAT
    pub request_stat_interrupt: bool,

    cycles: u32,
}

impl Gpu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            frame_buffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            lcdc: 0x91, stat: 0, scy: 0, scx: 0, ly: 0, lyc: 0,
            bgp: 0xFC, obp0: 0xFF, obp1: 0xFF, wy: 0, wx: 0,
            request_stat_interrupt: false,
            cycles: 0,
        }
    }

    pub fn read_vram(&self, addr: u16) -> u8 { self.vram[addr as usize] }
    pub fn write_vram(&mut self, addr: u16, val: u8) { self.vram[addr as usize] = val; }

    pub fn step(&mut self, cycles: u32) -> bool {
        // Reset de interrupción al inicio del paso
        self.request_stat_interrupt = false;

        if (self.lcdc & 0x80) == 0 {
            self.ly = 0;
            self.stat &= 0xFC;
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
                    
                    // IMPORTANTE: Checkear interrupción HBlank (Bit 3 de STAT)
                    if (self.stat & 0x08) != 0 {
                        self.request_stat_interrupt = true;
                    }
                    
                    self.render_scanline();
                }
            }
            Mode::HBlank => {
                if self.cycles >= 204 {
                    self.cycles -= 204;
                    self.ly += 1;
                    
                    // Chequeo de Coincidencia LY == LYC (Vital para Mario)
                    self.check_lyc();

                    if self.ly == 144 {
                        self.set_mode(Mode::VBlank);
                        // Interrupción VBlank (Bit 4 de STAT)
                        if (self.stat & 0x10) != 0 {
                            self.request_stat_interrupt = true;
                        }
                        frame_ready = true;
                    } else {
                        self.set_mode(Mode::OamSearch);
                        // Interrupción OAM (Bit 5 de STAT)
                        if (self.stat & 0x20) != 0 {
                            self.request_stat_interrupt = true;
                        }
                    }
                }
            }
            Mode::VBlank => {
                if self.cycles >= 456 {
                    self.cycles -= 456;
                    self.ly += 1;
                    self.check_lyc(); // Chequear también en VBlank

                    if self.ly > 153 {
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

    // --- Lógica LY=LYC (Mario Scroll Fix) ---
    fn check_lyc(&mut self) {
        if self.ly == self.lyc {
            self.stat |= 0x04; // Encender bit de coincidencia
            // Si la interrupción LYC (Bit 6) está habilitada, solicitarla
            if (self.stat & 0x40) != 0 {
                self.request_stat_interrupt = true;
            }
        } else {
            self.stat &= !0x04; // Apagar bit
        }
    }

    // --- Renderizado (Versión Final) ---

    fn render_scanline(&mut self) {
        if (self.lcdc & 0x01) != 0 { self.render_background(); }
        if (self.lcdc & 0x20) != 0 { self.render_window(); }
        if (self.lcdc & 0x02) != 0 { self.render_sprites(); }
    }

    fn render_background(&mut self) {
        let y = self.ly;
        let map_base: u16 = if (self.lcdc & 0x08) != 0 { 0x1C00 } else { 0x1800 };
        let use_unsigned = (self.lcdc & 0x10) != 0;
        let map_y = y.wrapping_add(self.scy);
        let tile_row = (map_y % 8) as u16;
        let tile_line_idx = (map_y / 8) as u16;

        for x in 0..SCREEN_WIDTH {
            let map_x = (x as u8).wrapping_add(self.scx);
            let tile_col = (map_x / 8) as u16;
            let tile_map_addr = map_base + (tile_line_idx * 32) + tile_col;
            let tile_id = self.vram[tile_map_addr as usize];
            let tile_data_addr = self.get_tile_data_addr(tile_id, use_unsigned);

            let byte1 = self.vram[(tile_data_addr + (tile_row * 2)) as usize];
            let byte2 = self.vram[(tile_data_addr + (tile_row * 2) + 1) as usize];
            
            let bit_idx = 7 - (map_x % 8);
            let lo = (byte1 >> bit_idx) & 1;
            let hi = (byte2 >> bit_idx) & 1;
            let color = self.get_color((hi << 1) | lo, self.bgp);

            self.set_pixel(x, y as usize, color);
        }
    }

    fn render_window(&mut self) {
        let wy = self.wy;
        if self.ly < wy { return; }

        let wx = self.wx;
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
        let sprite_height = if (self.lcdc & 0x04) != 0 { 16 } else { 8 };
        
        for i in 0..40 {
            let idx = i * 4;
            let y_pos = self.oam[idx] as i16 - 16;
            let x_pos = self.oam[idx+1] as i16 - 8;
            let tile_idx = self.oam[idx+2];
            let flags = self.oam[idx+3];

            if (self.ly as i16) < y_pos || (self.ly as i16) >= (y_pos + sprite_height) { continue; }

            let y_flip = (flags & 0x40) != 0;
            let x_flip = (flags & 0x20) != 0;
            let palette = if (flags & 0x10) != 0 { self.obp1 } else { self.obp0 };

            let mut line = (self.ly as i16 - y_pos) as u16;
            if y_flip { line = sprite_height as u16 - 1 - line; }
            let actual_tile_idx = if sprite_height == 16 { tile_idx & 0xFE } else { tile_idx };
            
            let tile_addr = (actual_tile_idx as u16 * 16) + (line * 2);
            let byte1 = self.vram[tile_addr as usize];
            let byte2 = self.vram[(tile_addr + 1) as usize];

            for pixel_x in 0..8 {
                let screen_x = x_pos + pixel_x;
                if screen_x < 0 || screen_x >= SCREEN_WIDTH as i16 { continue; }

                let bit_idx = if x_flip { pixel_x } else { 7 - pixel_x };
                let lo = (byte1 >> bit_idx) & 1;
                let hi = (byte2 >> bit_idx) & 1;
                let color_id = (hi << 1) | lo;

                if color_id == 0 { continue; } // Transparente

                let color = self.get_color(color_id, palette);
                self.set_pixel(screen_x as usize, self.ly as usize, color);
            }
        }
    }

    fn get_tile_data_addr(&self, tile_id: u8, use_unsigned: bool) -> u16 {
        if use_unsigned { (tile_id as u16) * 16 } else { (0x1000 + ((tile_id as i8) as i16 * 16)) as u16 }
    }

    fn get_color(&self, color_id: u8, palette: u8) -> [u8; 4] {
        match (palette >> (color_id * 2)) & 0x03 {
            0 => [0x9B, 0xBC, 0x0F, 0xFF],
            1 => [0x8B, 0xAC, 0x0F, 0xFF],
            2 => [0x30, 0x62, 0x30, 0xFF],
            3 => [0x0F, 0x38, 0x0F, 0xFF],
            _ => [0, 0, 0, 0],
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: [u8; 4]) {
        let offset = (y * SCREEN_WIDTH + x) * 4;
        self.frame_buffer[offset..offset+4].copy_from_slice(&color);
    }
    
    // Helpers Mode
    fn get_mode(&self) -> Mode {
        match self.stat & 0x03 { 0=>Mode::HBlank, 1=>Mode::VBlank, 2=>Mode::OamSearch, 3=>Mode::PixelTransfer, _=>unreachable!() }
    }
    fn set_mode(&mut self, mode: Mode) { self.stat = (self.stat & !0x03) | (mode as u8); }
}