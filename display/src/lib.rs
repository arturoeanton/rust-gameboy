// display/src/lib.rs

// Importamos crates externos para ventana y gráficos.
// - winit: Manejo de ventanas multiplataforma.
// - pixels: Renderizado de buffers de píxeles eficiente (hardware accelerated).
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use pixels::{Pixels, SurfaceTexture};
use winit_input_helper::WinitInputHelper;

use cpu::Cpu;
use memory::Bus;
use joypad::Button;
use gpu::{SCREEN_WIDTH, SCREEN_HEIGHT};

/// Función principal que toma el control del emulador.
/// Recibe la CPU y el Bus con propiedad (ownership), consumiéndolos.
/// Esto garantiza que nadie más pueda modificarlos fuera del bucle.
pub fn run(mut cpu: Cpu, mut bus: Bus) {
    // 1. Configurar la ventana (Window)
    // EventLoop maneja los mensajes del SO (clics, teclas, redibujado).
    let event_loop = EventLoop::new();
    
    // WindowBuilder: Patrón Builder para configurar la ventana.
    // .build() devuelve un Result, usamos .unwrap() para panickear si falla.
    let window = WindowBuilder::new()
        .with_title("Rust GameBoy Emulator")
        // Escalamos x3 para ver algo en pantallas modernas (160x144 es minúsculo).
        .with_inner_size(winit::dpi::LogicalSize::new(SCREEN_WIDTH as f64 * 3.0, SCREEN_HEIGHT as f64 * 3.0)) 
        .build(&event_loop)
        .unwrap();

    // 2. Configurar el buffer de píxeles
    // Bloque delimitado {} para limitar el scope de variables temporales.
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture).unwrap()
    };

    // Helper para simplificar el manejo de input (teclado).
    let mut input = WinitInputHelper::new();

    // 3. El Bucle Principal (Game Loop)
    // event_loop.run toma el control del hilo principal (necesario en macOS).
    // El closure 'move |...|' captura variables del entorno (cpu, bus, pixels) moviéndolas dentro.
    event_loop.run(move |event, _, control_flow| {
        // Por defecto, seguimos corriendo.
        // *control_flow es como asignar a un puntero en Go.
        *control_flow = ControlFlow::Poll;

        // Manejo de eventos de ventana (cerrar, redimensionar)
        if input.update(&event) {
            // Tecla Escape o botón Cerrar -> Salir
            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Manejo del Joypad (Mapeo de teclas PC -> Game Boy)
            handle_input(&input, &mut bus);

            // Redimensionar buffer si la ventana cambia de tamaño
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height).unwrap();
            }
        }

        // 4. Renderizado: Winit emite RedrawRequested cuando toca dibujar.
        if let Event::RedrawRequested(_) = event {
            // A. Ejecutar un frame completo de la CPU
            // La Game Boy corre a ~59.7 Hz (aprox 60).
            // Un frame son exactamente 70224 ciclos de reloj (T-Cycles) o 17556 M-Cycles.
            // Aquí simplificamos corriendo esa cantidad de ciclos de golpe.
            let mut cycles_spent = 0;
            const CYCLES_PER_FRAME: u32 = 70224; // M-Cycles * 4 o T-Cycles directos? 
            // Nota: Nuestro CPU devuelve 'M-Cycles' (1 op = 1..6 ciclos).
            // En hardware real clock es 4.19MHz. M-Cycle es 1.05MHz.
            // Si Gpu/Timer esperan T-Cycles, multiplicaremos x4 internamente.
            // Aquí asumimos cycles_spent es en T-Cycles.
            
            while cycles_spent < CYCLES_PER_FRAME {
                // cpu.step devuelve M-Cycles (ej: 1 para NOP).
                let m_cycles = cpu.step(&mut bus);
                
                // Convertimos a T-Cycles (Reloj del sistema) para precisión.
                let t_cycles = m_cycles * 4;
                cycles_spent += t_cycles;
                
                // Actualizar GPU (PPU)
                // Devuelve true si acaba de entrar en V-Blank (frame listo).
                let frame_ready = bus.gpu.step(t_cycles);
                
                // Actualizar Timer (DIV, TIMA)
                bus.step_timer(m_cycles); // El timer suele contar M-Cycles internamente

                // Interrupción LCD STAT
                if bus.gpu.request_stat_interrupt {
                    bus.interrupt_flag |= 0x02; // Bit 1
                }
                
                // Si la GPU terminó de dibujar la pantalla
                if frame_ready {
                    // Copiar el buffer linear de la GPU al Texture de la ventana
                    let frame = pixels.frame_mut();
                    frame.copy_from_slice(&bus.gpu.frame_buffer);
                    
                    // Solicitar interrupción VBlank a la CPU (Bit 0)
                    bus.interrupt_flag |= 0x01; 
                }
            }

            // B. Dibujar en pantalla (Swap buffers)
            if pixels.render().is_err() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }
        
        // Solicitar redibujado inmediato para el siguiente frame (VSync lo limitará).
        window.request_redraw();
    });
}

/// Helper para mapear teclado moderno a botones de GB.
/// Recibe referencia mutable al Bus porque necesita modificar 'joypad'.
fn handle_input(input: &WinitInputHelper, bus: &mut Bus) {
    // Teclas presionadas
    if input.key_pressed(VirtualKeyCode::Z) { bus.joypad.key_down(Button::A); }
    if input.key_pressed(VirtualKeyCode::X) { bus.joypad.key_down(Button::B); }
    if input.key_pressed(VirtualKeyCode::Return) { bus.joypad.key_down(Button::Start); }
    if input.key_pressed(VirtualKeyCode::Back) { bus.joypad.key_down(Button::Select); } // Backspace
    if input.key_pressed(VirtualKeyCode::Up) { bus.joypad.key_down(Button::Up); }
    if input.key_pressed(VirtualKeyCode::Down) { bus.joypad.key_down(Button::Down); }
    if input.key_pressed(VirtualKeyCode::Left) { bus.joypad.key_down(Button::Left); }
    if input.key_pressed(VirtualKeyCode::Right) { bus.joypad.key_down(Button::Right); }

    // Teclas soltadas
    if input.key_released(VirtualKeyCode::Z) { bus.joypad.key_up(Button::A); }
    if input.key_released(VirtualKeyCode::X) { bus.joypad.key_up(Button::B); }
    if input.key_released(VirtualKeyCode::Return) { bus.joypad.key_up(Button::Start); }
    if input.key_released(VirtualKeyCode::Back) { bus.joypad.key_up(Button::Select); }
    if input.key_released(VirtualKeyCode::Up) { bus.joypad.key_up(Button::Up); }
    if input.key_released(VirtualKeyCode::Down) { bus.joypad.key_up(Button::Down); }
    if input.key_released(VirtualKeyCode::Left) { bus.joypad.key_up(Button::Left); }
    if input.key_released(VirtualKeyCode::Right) { bus.joypad.key_up(Button::Right); }
}