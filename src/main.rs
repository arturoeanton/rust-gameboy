use std::env; // Biblioteca estándar para interactuar con el entorno (similar al paquete "os" en Go)
use std::fs;  // Biblioteca estándar para sistema de archivos ("io/ioutil" o "os" en Go)
use std::process; // Para controlar el proceso del sistema (exit codes)

// --- MODULOS ---
// En Rust, estos 'use' traen items de otros crates (bibliotecas) al scope actual.
// Es similar a los imports en Go, pero Rust es más explícito con la visibilidad.
use cpu::Cpu;
use memory::Bus;
use mbc::new_cartridge;

fn main() {
    // 1. Leer argumentos de la línea de comandos
    // 'env::args()' devuelve un iterador.
    // '.collect()' consume el iterador y lo transforma en una colección, aquí un Vec<String>.
    // En Go: args := os.Args
    let args: Vec<String> = env::args().collect();

    // Verificación de longitud. 
    // Nota: args[0] es el nombre del ejecutable, igual que en C o Go.
    if args.len() < 2 {
        eprintln!("Uso: {} <archivo_rom.gb>", args[0]);
        process::exit(1);
    }
    
    // Referencia inmutable (&String) al nombre del archivo.
    // En Rust, intentamos no copiar strings si no es necesario.
    let filename = &args[1];

    println!("Cargando ROM: {}", filename);

    // 2. Leer el archivo binario del disco
    // 'fs::read' devuelve un Result<Vec<u8>, std::io::Error>.
    // Result es un enum: Ok(data) o Err(error). No existen excepciones, ni nil.
    // En Go usaríamos: data, err := os.ReadFile(...)
    let rom_data = match fs::read(filename) {
        Ok(data) => data, // Si todo sale bien, extraemos 'data'
        Err(e) => {
            // 'eprintln!' imprime a stderr.
            eprintln!("Error leyendo el archivo: {}", e);
            process::exit(1);
        }
    };

    // 'rom_data' es ahora dueño (owner) del vector de bytes.
    println!("Tamaño de ROM: {} bytes", rom_data.len());

    // 3. Ensamblaje de componentes (Hardware Wiring)
    
    // A. Crear el cartucho (Mbc) correcto
    // Llamamos a una función factoría que devuelve un Box<dyn Mbc>.
    // - Box<T>: Un puntero inteligente que aloja datos en el Heap (necesario para polimorfismo dinámico).
    // - dyn Mbc: "Trait Object". Similar a una interface en Go. Significa "cualquier struct que implemente Mbc".
    let cartucho = new_cartridge(rom_data);

    // B. Insertar cartucho en el Bus de memoria
    // Movemos 'cartucho' dentro del Bus. 'main' pierde la posesión de 'cartucho'.
    // Si intentáramos usar 'cartucho' después de esta línea, el compilador daría error.
    let bus = Bus::new(cartucho);

    // C. Conectar la CPU al sistema
    let cpu = Cpu::new();

    println!("Sistema ensamblado. Iniciando emulación...");

    // 4. Transferir control al sistema de Display (Bucle infinito)
    // El sistema de display manejará el bucle de eventos (input/render).
    // Le transferimos la propiedad (ownership) de 'cpu' y 'bus'.
    display::run(cpu, bus);
}