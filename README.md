# Rust Game Boy Emulator 🕹️

[English](#english) | [Español](#español)

---

## English

### 🎯 About the Project
This is a **Game Boy (DMG-01) emulator** written in Rust, designed with a strong focus on **education and clarity**. 

**Important:** This project is **not** intended for production or to compete with professional emulators like BGB or SameBoy. It was created as a **curiosity and educational experiment** to explore:
*   How CPU opcodes work at a low level.
*   The architecture of 8-bit systems (SM83, PPU, Memory Bus).
*   Applying Rust's safety and performance features in system-level programming.

The source code is heavily commented (in Spanish) to help developers, especially those coming from languages like Go, understand the intricacies of both Rust and emulator development.

### 🚀 Status
*   **CPU:** Complete SM83 implementation (all 8-bit and 16-bit loads, arithmetic, and control flow instructions) with dense educational comments.
*   **PPU:** Functional background and sprite rendering. Supports window and basic palette handling.
*   **Memory:** Full Memory Bus mapping, including DMA implementation for OAM.
*   **MBC:** Support for ROM Only (e.g., Tetris), MBC1 (e.g., Super Mario Land), and basic MBC3 structure.
*   **Audio:** Not yet implemented.

### 🎮 Supported Games (Tested)
The following games (available in the `roms/` directory) have been tested and boot successfully:

*   Best of the Best - Championship Karate
*   Captain Tsubasa VS
*   Donkey Kong Land III
*   DuckTales
*   Felix the Cat
*   FIFA Soccer 96 & 97
*   Metroid II - Return of Samus
*   Ninja Gaiden Shadow
*   Nintendo World Cup
*   Prince of Persia
*   Street Fighter II
*   Super Mario Land 1 & 2
*   Tetris
*   Tiny Toon Adventures
*   Tom & Jerry

### 🛠️ How to run
Make sure you have [Rust](https://rustup.rs/) installed.
```bash
cargo run --release -- "roms/Super Mario Land (World).gb"
```

---

## Español

### 🎯 Sobre el Proyecto
Este es un **emulador de Game Boy (DMG-01)** escrito en Rust, diseñado con un fuerte enfoque en **educación y claridad**.

**Importante:** Este proyecto **no** tiene fines productivos ni pretende competir con emuladores profesionales. Fue creado como una **curiosidad y experimento educativo** para explorar:
*   El funcionamiento de los opcodes de la CPU a bajo nivel.
*   La arquitectura de sistemas de 8 bits (SM83, PPU, Bus de Memoria).
*   La aplicación de la seguridad y rendimiento de Rust en programación de sistemas.

El código fuente está exhaustivamente comentado en español para ayudar a desarrolladores (especialmente aquellos que vienen de lenguajes como Go) a entender tanto los detalles de Rust como el desarrollo de emuladores.

### 🚀 Estado Actual
*   **CPU:** Implementación completa del set de instrucciones SM83 (cargas de 8/16 bits, aritmética, saltos, etc.) con comentarios educativos detallados.
*   **PPU:** Renderizado funcional de Background y Sprites (Objetos). Soporte básico de ventana y paletas.
*   **Memoria:** Mapeo completo del Bus, incluyendo implementación de DMA para OAM.
*   **MBC:** Soporte para cartuchos ROM Only (ej. Tetris), MBC1 (ej. Super Mario Land) y estructura básica para MBC3.
*   **Audio:** Aún no implementado.

### 🎮 Juegos Soportados (Probados)
Los siguientes juegos (disponibles en la carpeta `roms/`) han sido probados y arrancan exitosamente:

*   Best of the Best - Championship Karate
*   Captain Tsubasa VS
*   Donkey Kong Land III
*   DuckTales
*   Felix the Cat
*   FIFA Soccer 96 & 97
*   Metroid II - Return of Samus
*   Ninja Gaiden Shadow
*   Nintendo World Cup
*   Prince of Persia
*   Street Fighter II
*   Super Mario Land 1 & 2
*   Tetris
*   Tiny Toon Adventures
*   Tom & Jerry

### 🛠️ Cómo ejecutar
Asegúrate de tener [Rust](https://rustup.rs/) instalado.
```bash
cargo run --release -- "roms/Super Mario Land (World).gb"
```

---

*Hecho con ❤️ por programadores curiosos.*