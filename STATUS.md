# Estado del Proyecto Rust Game Boy 🚀

Este documento detalla el estado actual de desarrollo del emulador, desglosado por componentes.

## 1. CPU (Sharp SM83)
**Estado: ✅ Completado** aka (Casi perfecto)

El núcleo del emulador está completamente funcional. Se ha implementado la totalidad del set de instrucciones del procesador SM83 (una variante del Z80) utilizada por la Game Boy.

*   **Instrucciones Base:** Todas las operaciones de carga (LD), aritmética (ADD, SUB, XOR...), lógica y control de flujo (JP, CALL, RET) están implementadas.
*   **Prefijo CB:** Implementadas todas las instrucciones extendidas (RLC, RRC, SWAP, BIT, SET, RES).
*   **Flags:** La gestión de banderas (Zero, Subtraction, Half-Carry, Carry) ha sido verificada para la mayoría de operaciones.
*   **Interrupciones:** Sistema de interrupciones funcional (V-Blank, LCD Stat, Timer, Serial, Joypad) con prioridades correctas y manejo de registros `IE` (Enable) e `IF` (Flag).
*   **HALT/STOP:** Soporte básico para modos de bajo consumo.

## 2. PPU (Unidad de Procesamiento de Gráficos)
**Estado: ⚠️ Funcional (Básico)**

La GPU permite jugar a la mayoría de juegos comerciales, pero carece de precisión de ciclo (pixel-perfect accuracy).

*   **Background:** Renderizado de mapas de tiles (modos 0x9800/0x9C00) con soporte de Scroll (SCX, SCY).
*   **Sprites:** Soporte para objetos de 8x8 y 8x16, con soporte de Flip X/Y.
*   **Window:** Funcionalidad de ventana implementada (WX, WY).
*   **Paletas:** Soporte para paletas monocromáticas (BGP, OBP0, OBP1).
*   **Timing:** La máquina de estados del PPU (HBlank, VBlank, OAM Search, Pixel Transfer) está simulada, pero no es perfectamente precisa en tiempos.

## 3. Memoria y Bus
**Estado: ✅ Completado**

El sistema de memoria interconecta correctamente todos los componentes.

*   **Mapa de Memoria:** Direccionamiento correcto de ROM, VRAM, WRAM, OAM, I/O y HRAM.
*   **DMA (Direct Memory Access):** Implementado el mecanismo de transferencia rápida para la memoria de objetos (OAM DMA), vital para todos los juegos.
*   **Echo RAM:** Redirección básica implementada para compatibilidad.

## 4. Cartuchos (MBC)
**Estado: ⚠️ Parcial**

El soporte de cartuchos cubre los casos más comunes.

*   **ROM ONLY:** Juegos simples como *Tetris* funcionan perfectamente.
*   **MBC1:** Soporte inicial para cambio de bancos (Banking) de ROM y RAM. Juegos como *Super Mario Land* funcionan.
*   **MBC3:** Estructura básica presente, pero sin reloj en tiempo real (RTC). Juegos como *Pokémon Red/Blue* podrían arrancar pero fallar al guardar/cargar o usar features avanzados.

## 5. Entrada (Input)
**Estado: ✅ Completado**

*   **Joypad:** Mapeo completo de teclas de PC al control de Game Boy. Soporte para interrupciones de hardware al presionar teclas.

## 6. Audio (APU)
**Estado: ❌ No Implementado**

Actualmente el emulador es **mudo**. No se ha implementado la unidad de procesamiento de audio (canales de onda cuadrada, ruido, etc.).

## 7. Documentación y Educación
**Estado: 🌟 Excelente**

El proyecto destaca por su documentación inline. Todo el código fuente crítico (`main`, `cpu`, `memory`, `gpu`, etc.) ha sido comentado exhaustivamente en español, explicando:
*   Qué hace cada línea de código.
*   Conceptos de emulación (por qué hacemos esto).
*   Conceptos de Rust (por qué usamos `Box`, `Option`, `match`, etc.) comparados con otros lenguajes como Go.

---
*Documento actualizado automáticamente el 24 de Diciembre de 2024.*
