// joypad/src/lib.rs

/// Enumeración para identificar los botones.
/// Derivamos Debug, Clone, Copy y PartialEq para usarlos fácilmente.
/// - Copy: Permite pasar el botón por valor sin mover la propiedad (ownership).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Button {
    Right, Left, Up, Down,
    A, B, Select, Start,
}

/// Representación del Hardware de Entrada.
/// El Game Boy lee los botones en una matriz de 2x4.
pub struct Joypad {
    // Estado booleano de cada botón (true = presionado).
    right: bool, left: bool, up: bool, down: bool,
    a: bool, b: bool, select: bool, start: bool,

    // Registro interno de selección (Bits 4 y 5 de 0xFF00).
    // La CPU escribe aquí para elegir qué fila leer:
    // - Bit 4 = 0: Selecciona Direcciones.
    // - Bit 5 = 0: Selecciona Acciones (A, B, Start, Select).
    selection: u8,

    // Solicitud de interrupción.
    // 'pub' permite que el Bus o Display lean esto directamente.
    pub interrupt_request: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            right: false, left: false, up: false, down: false,
            a: false, b: false, select: false, start: false,
            selection: 0xFF, // Inicialmente 1111 1111 (nada seleccionado)
            interrupt_request: false,
        }
    }

    /// Llamado cuando el usuario presiona una tecla (Evento UI).
    /// '&mut self' permite modificar el estado interno.
    pub fn key_down(&mut self, btn: Button) {
        match btn {
            Button::Right => self.right = true,
            Button::Left => self.left = true,
            Button::Up => self.up = true,
            Button::Down => self.down = true,
            Button::A => self.a = true,
            Button::B => self.b = true,
            Button::Select => self.select = true,
            Button::Start => self.start = true,
        }
        // Solicitar interrupción Joypad (INT 60h).
        // En hardware real solo ocurre en flanco de bajada (High->Low).
        self.interrupt_request = true;
    }

    /// Llamado cuando el usuario suelta una tecla.
    pub fn key_up(&mut self, btn: Button) {
        match btn {
            Button::Right => self.right = false,
            Button::Left => self.left = false,
            Button::Up => self.up = false,
            Button::Down => self.down = false,
            Button::A => self.a = false,
            Button::B => self.b = false,
            Button::Select => self.select = false,
            Button::Start => self.start = false,
        }
    }

    /// Lectura del registro P1 (0xFF00).
    /// La lógica es "Active Low" (0 = Seleccionado/Presionado).
    pub fn read(&self) -> u8 {
        // Empezamos con 0xCF (1100 1111).
        // Bits 6-7 siempre 1 (no usados).
        // Bits 4-5 copiamos la selección actual.
        // Bits 0-3 asumimos 1 (no presionado) inicialmente.
        let mut value = self.selection | 0xCF;

        // Si bit 4 es 0, leer DIRECCIONES
        if (self.selection & 0x10) == 0 {
            // Si está presionado (true), ponemos el bit a 0.
            // A &= !MASK -> apaga los bits de la máscara.
            if self.right { value &= !0x01; }
            if self.left  { value &= !0x02; }
            if self.up    { value &= !0x04; }
            if self.down  { value &= !0x08; }
        }

        // Si bit 5 es 0, leer ACCIONES
        if (self.selection & 0x20) == 0 {
            if self.a      { value &= !0x01; }
            if self.b      { value &= !0x02; }
            if self.select { value &= !0x04; }
            if self.start  { value &= !0x08; }
        }

        value
    }

    /// Escritura en P1 (0xFF00).
    /// La CPU selecciona qué quiere leer escribiendo ceros en bits 4 o 5.
    pub fn write(&mut self, val: u8) {
        // Solo permitimos escribir bits 4-5.
        // Los bits 0-3 son Read-Only (estado de botones).
        self.selection = val & 0x30;
    }
}