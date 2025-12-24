// joypad/src/lib.rs

/// Enumeración para identificar los botones desde la interfaz gráfica (UI)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Button {
    Right, Left, Up, Down,
    A, B, Select, Start,
}

/// Representación del hardware de entrada (Matriz de 2x4).
/// El registro P1 (0xFF00) controla qué "fila" de la matriz se lee.
pub struct Joypad {
    // Estado interno de los botones (true = presionado por el usuario)
    right: bool,
    left: bool,
    up: bool,
    down: bool,
    a: bool,
    b: bool,
    select: bool,
    start: bool,

    // Registro de Selección (Bits 4 y 5 de 0xFF00)
    // Bit 4 = 0 -> Selecciona direcciones (Right, Left, Up, Down)
    // Bit 5 = 0 -> Selecciona acciones (A, B, Select, Start)
    selection: u8,

    // Bandera para indicar que se debe disparar una interrupción (Joypad IRQ)
    pub interrupt_request: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            right: false, left: false, up: false, down: false,
            a: false, b: false, select: false, start: false,
            selection: 0xFF, // Inicialmente nada seleccionado (todo en 1)
            interrupt_request: false,
        }
    }

    /// Evento de tecla presionada (Llamado desde Main/Display)
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
        // En hardware real, la interrupción se dispara en el flanco de bajada (high -> low).
        // Simplificamos solicitando interrupción siempre que se presiona algo.
        self.interrupt_request = true;
    }

    /// Evento de tecla soltada
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

    /// Lectura del registro P1 (0xFF00)
    /// La CPU lee este registro para saber qué botones están presionados
    /// basándose en la selección previa que hizo.
    pub fn read(&self) -> u8 {
        // Comenzamos con 0xCF (1100 1111) -> Bits 6 y 7 siempre son 1 (unused)
        // Bit 4 y 5 vienen de la selección actual.
        // Bits 0-3 son los botones (inicialmente 1 = sueltos).
        let mut value = self.selection | 0xCF;

        // Lógica Active Low: Si la selección es 0 (activo), miramos los botones.
        
        // Si Bit 4 es 0, la CPU quiere leer DIRECCIONES
        if (self.selection & 0x10) == 0 {
            if self.right { value &= !0x01; } // Ponemos el bit 0 a 0 (presionado)
            if self.left  { value &= !0x02; }
            if self.up    { value &= !0x04; }
            if self.down  { value &= !0x08; }
        }

        // Si Bit 5 es 0, la CPU quiere leer ACCIONES
        if (self.selection & 0x20) == 0 {
            if self.a      { value &= !0x01; }
            if self.b      { value &= !0x02; }
            if self.select { value &= !0x04; }
            if self.start  { value &= !0x08; }
        }

        value
    }

    /// Escritura en el registro P1 (0xFF00)
    /// La CPU escribe aquí para decir "Quiero leer las flechas" o "Quiero leer los botones A/B".
    pub fn write(&mut self, val: u8) {
        // Solo nos importan los bits 4 y 5 para la selección.
        // Los bits 0-3 son de solo lectura (read-only) desde la perspectiva de escritura.
        self.selection = val & 0x30;
    }
}