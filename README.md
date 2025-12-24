# Rust Game Boy Emulator 🕹️

[English](#english) | [Español](#español)

---

## English

### 🎯 About the Project
This is a **simple Game Boy (DMG-01) emulator** written in Rust. 

**Important:** This project is **not** intended for production or to compete with professional emulators like BGB or SameBoy. It was created as a **curiosity and educational experiment** to explore:
*   How CPU opcodes work at a low level.
*   The architecture of 8-bit systems (SM83, PPU, Memory Bus).
*   Applying Rust's safety and performance in system-level programming.

### 🚀 Status
*   **CPU:** Almost complete SM83 implementation with dense comments in Spanish for educational purposes.
*   **PPU:** Basic background and sprite rendering implemented.
*   **Memory:** Full Bus mapping, including DMA for OAM.
*   **MBC:** Basic support for ROM and MBC1/MBC3 (via the `mbc` module).

### 🛠️ How to run
Make sure you have [Rust](https://rustup.rs/) installed.
```bash
cargo run --release -- path/to/rom.gb
```

---

## Español

### 🎯 Sobre el Proyecto
Este es un **emulador sencillo de Game Boy (DMG-01)** escrito en Rust.

**Importante:** Este proyecto **no** tiene fines productivos ni pretende competir con emuladores profesionales. Fue creado como una **curiosidad y experimento educativo** para explorar:
*   El funcionamiento de los opcodes de la CPU a bajo nivel.
*   La arquitectura de sistemas de 8 bits (SM83, PPU, Bus de Memoria).
*   La aplicación de la seguridad y rendimiento de Rust en programación de sistemas.

### 🚀 Estado Actual
*   **CPU:** Implementación casi completa del SM83 con comentarios detallados en español para facilitar el aprendizaje.
*   **PPU:** Renderizado básico de fondo (background) y objetos (sprites).
*   **Memoria:** Mapeo completo del Bus, incluyendo transferencias DMA para OAM.
*   **MBC:** Soporte básico para ROM Only, MBC1 y MBC3.

### 🛠️ Cómo ejecutar
Asegúrate de tener [Rust](https://rustup.rs/) instalado.
```bash
cargo run --release -- ruta/al/archivo.gb
```

---

*Hecho con ❤️ por programadores curiosos.*