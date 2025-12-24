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
/// Recibe la CPU y el Bus ya ensamblados y arranca el bucle infinito.
pub fn run(mut cpu: Cpu, mut bus: Bus) {
    // 1. Configurar la ventana (Window)
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Rust GameBoy Emulator")
        .with_inner_size(winit::dpi::LogicalSize::new(SCREEN_WIDTH as f64 * 3.0, SCREEN_HEIGHT as f64 * 3.0)) // Zoom x3
        .build(&event_loop)
        .unwrap();

    // 2. Configurar el buffer de píxeles
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture).unwrap()
    };

    let mut input = WinitInputHelper::new();

    // 3. El Bucle Principal (Game Loop)
    event_loop.run(move |event, _, control_flow| {
        // Manejo de eventos de ventana (cerrar, redimensionar)
        if input.update(&event) {
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Manejo del Joypad (Mapeo de teclas PC -> Game Boy)
            handle_input(&input, &mut bus);

            // Redimensionar si el usuario cambia el tamaño de ventana
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height).unwrap();
            }
        }

        // 4. Renderizado: Solo dibujamos cuando la ventana lo pide
        if let Event::RedrawRequested(_) = event {
            // A. Ejecutar un frame completo de la CPU (aprox 70224 ciclos)
            //    Esto asegura que el juego corra a 60 FPS
            let mut cycles_spent = 0;
            const CYCLES_PER_FRAME: u32 = 70224;
            
            while cycles_spent < CYCLES_PER_FRAME {
                let cycles = cpu.step(&mut bus);
                cycles_spent += cycles;

               
                
                // Actualizar GPU 
                let frame_ready = bus.gpu.step(cycles);
                 // Actualizar Timer ---
                bus.step_timer(cycles);

                // --- ¡FIX MARIO! Conectar interrupción LCD STAT ---
                if bus.gpu.request_stat_interrupt {
                    bus.interrupt_flag |= 0x02; // Bit 1: LCD STAT
                }
                
                // Si la GPU dice "V-Blank", tenemos una imagen lista
                if frame_ready {
                    // Copiar el buffer de nuestra GPU al buffer de la ventana
                    let frame = pixels.frame_mut();
                    frame.copy_from_slice(&bus.gpu.frame_buffer);
                    
                    // Manejo de interrupciones (VBlank Interrupt)
                    bus.interrupt_flag |= 0x01; 
                }
            }

            // B. Dibujar en pantalla
            if pixels.render().is_err() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }
        
        // Solicitar redibujado continuo
        window.request_redraw();
    });
}

/// Helper para mapear teclado moderno a botones de GB
fn handle_input(input: &WinitInputHelper, bus: &mut Bus) {
    // Teclas presionadas
    if input.key_pressed(VirtualKeyCode::Z) { bus.joypad.key_down(Button::A); }
    if input.key_pressed(VirtualKeyCode::X) { bus.joypad.key_down(Button::B); }
    if input.key_pressed(VirtualKeyCode::Return) { bus.joypad.key_down(Button::Start); }
    if input.key_pressed(VirtualKeyCode::Back) { bus.joypad.key_down(Button::Select); }
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