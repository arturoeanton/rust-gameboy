use std::env;
use std::fs;
use std::process;

use cpu::Cpu;
use memory::Bus;
use mbc::new_cartridge;

fn main() {
    // 1. Leer argumentos de la línea de comandos
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <archivo_rom.gb>", args[0]);
        process::exit(1);
    }
    let filename = &args[1];

    println!("Cargando ROM: {}", filename);

    // 2. Leer el archivo binario del disco
    let rom_data = match fs::read(filename) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error leyendo el archivo: {}", e);
            process::exit(1);
        }
    };

    println!("Tamaño de ROM: {} bytes", rom_data.len());

    // 3. Ensamblaje de componentes (Hardware Wiring)
    
    // A. Crear el cartucho (Mbc) correcto
    let cartucho = new_cartridge(rom_data);

    // B. Insertar cartucho en el Bus de memoria
    let bus = Bus::new(cartucho);

    // C. Conectar la CPU al sistema
    let cpu = Cpu::new();

    println!("Sistema ensamblado. Iniciando emulación...");

    // 4. Transferir control al sistema de Display (Bucle infinito)
    display::run(cpu, bus);
}