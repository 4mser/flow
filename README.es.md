<p align="center">
  <img src="assets/flow/hero.svg" width="100%" alt="FLOW: un teclado y un mouse para todas tus máquinas. Mac, Windows y Linux, cifrado y directo, construido sobre Beamer de Kalkman Code">
</p>

<p align="center">
  <a href="README.md"><b>Read in English</b></a> &nbsp;·&nbsp;
  <a href="#-07-construido-sobre-beamer"><b>Construido sobre Beamer</b></a> &nbsp;·&nbsp;
  <a href="WIRE.md"><b>Protocolo</b></a> &nbsp;·&nbsp;
  <a href="SECURITY.md"><b>Seguridad</b></a>
</p>

FLOW convierte los computadores de tu escritorio en un solo espacio de trabajo. Empujas el puntero fuera del borde de tu Mac y sigue en el PC de al lado, con el teclado, los atajos y el portapapeles incluidos. Las máquinas hablan directamente entre ellas por tu red, cifradas, sin cuenta y sin servidor en medio.

FLOW nació como un prototipo mío en Rust para compartir el portapapeles entre PCs. La parte difícil, capturar la entrada a nivel del sistema operativo y pasarla de forma segura, no iba a quedar bien rápido. [Beamer](https://github.com/kalkman-code/beamer), de Toby Kalkman, ya lo había resuelto con cuidado y en código abierto. Por eso FLOW ahora es un fork de Beamer: todo lo que hace Beamer, más la idea con la que nació FLOW. **Varias personas, varias máquinas, un escritorio, y cada mano con su propio color.**

<p align="center">
  <img src="assets/flow/crossing.gif" width="100%" alt="Un puntero empuja el borde derecho de un MacBook hasta que el borde se ilumina, cruza al PC con Windows a la misma altura, y una etiqueta roja con el nombre Nico's MacBook lo sigue mientras el PC indica Driven by Nico's MacBook">
</p>

### `// 01` Qué hace

| | |
|---|---|
| **Empujar a través del borde** | El puntero sale por el borde que mira a la otra máquina y llega al mismo punto del borde opuesto. El borde resiste, se ilumina mientras empujas y cede. |
| **O con una tecla** | Toca dos veces o mantén una tecla para cambiar sin mover el puntero. Agita el mouse para volver a casa. |
| **En ambos sentidos** | El Mac maneja el PC y el PC maneja el Mac. Mac con Mac y PC con PC también. Un interruptor por dirección. |
| **El portapapeles viaja contigo** | Texto e imágenes, en ambos sentidos, en el momento en que cruzas. |
| **Tus atajos siguen funcionando** | Entre un Mac y Windows, Cmd pasa a Ctrl y Option pasa a Alt, así que Cmd+C también copia en el PC. |
| **Teclas multimedia y gestos** | Reproducir, pausar y el volumen cruzan. Con tres y cuatro dedos en el trackpad del Mac manejas la Vista de tareas y los escritorios virtuales de Windows. |
| **Varias máquinas** | Empareja cada máquina con las demás y ordénalas en un mapa: bordes, partes de un borde, esquinas y hasta el notch del MacBook. |
| **Cifrado y directo** | Empareja una vez con un código de seis dígitos. Cada enlace después va cifrado de extremo a extremo, con claves nuevas cada vez. |
| **La etiqueta de visitante** *(nuevo en FLOW)* | Mientras otra máquina maneja la tuya, una etiqueta con el color de esa máquina sigue al puntero y dice de quién es la mano. |

Mac (macOS 13 o posterior), Windows 10 y 11, y Linux como Flatpak (probado en GNOME con Wayland; KDE y X11 todavía son experimentales). Gratis y de código abierto bajo GPL-3.0.

### `// 02` Empujar a través del borde

Esta es la parte que hace que se sienta bien, y viene directo de Beamer. Tocar el borde no hace nada por sí solo. Empujar más allá acumula presión, que se descarga cuando paras, así el puntero nunca salta a la otra pantalla por accidente cuando vas a la barra de desplazamiento. Si sigues empujando, el borde cede: el puntero llega a la otra máquina a la misma altura, con una animación breve para que nunca lo tengas que buscar.

- **Eliges el tramo.** Un borde completo, solo una parte, una esquina o hacia arriba por el notch.
- **Eliges la fuerza.** La resistencia es un ajuste, desde un toque suave hasta un empujón decidido.
- **Eliges el estilo.** Glow, Beam y Aperture, o los efectos completos (Membrane, Sparks, Instrument, Folio, Selvedge) con paquetes de color, tamaño, duración y cuadros por segundo.
- **No estorba.** Un arrastre que llega al borde sigue siendo un arrastre, y una app a pantalla completa puede retener sus bordes.

El modelo es puro y tiene sus propios tests: [`core/return_edge.py`](core/return_edge.py).

### `// 03` Portapapeles y emparejamiento

<table>
  <tr>
    <td width="50%"><img src="assets/flow/clipboard.gif" width="100%" alt="Se selecciona un comando y se copia con Cmd C en el MacBook, el puntero cruza al PC con Windows y Ctrl V lo pega en PowerShell"></td>
    <td width="50%"><img src="assets/flow/pairing.gif" width="100%" alt="El PC con Windows muestra el código 482 913, el MacBook lo escribe en seis casillas, ambos muestran Paired y entre ellos fluyen tramas selladas con X25519 y ChaCha20-Poly1305"></td>
  </tr>
  <tr>
    <td><b>Copias en uno, pegas en el otro.</b> El portapapeles viaja con el puntero cuando cruzas, ni antes ni a todos: solo lo recibe la máquina en la que estás.</td>
    <td><b>Seis dígitos una vez, cifrado siempre.</b> Una máquina muestra un código durante un minuto y la otra lo escribe. El código nunca viaja; ambos lados demuestran que lo conocen (CPace).</td>
  </tr>
</table>

### `// 04` Multijugador: la visión

<p align="center">
  <img src="assets/flow/multiplayer.gif" width="100%" alt="Visión: una pantalla compartida con tres punteros a la vez, uno rojo con el nombre Nico, uno azul con Ana y uno verde con Tomás, cada uno señalando una tarjeta distinta, con sus máquinas listadas abajo">
</p>

Un KVM mueve la entrada de una persona entre máquinas. FLOW quiere también lo otro: varias personas, cada una en su máquina, todas visibles en la pantalla que importa en ese momento, cada puntero con el color y el nombre de su dueño. Señalar una línea en la pantalla de un compañero sin quitarle el mouse. Guiar a alguien por un ajuste mientras mantiene el control.

Un sistema operativo tiene un solo puntero real, y la regla de Beamer de que una máquina obedece a un solo dueño a la vez es lo que mantiene la entrada segura. Por eso FLOW construye el multijugador por capas, sin pelear con esa regla:

| Paso | Qué hace | Estado |
|---|---|---|
| **Etiqueta de visitante** | Mientras otra máquina maneja la tuya, el puntero real lleva una etiqueta con el nombre y el color de esa máquina. El mismo color en todas, derivado de su id, así que no viaja nada nuevo por la red. | Hecha para Mac y Windows, falta probarla en equipos reales |
| **Tu color** | Elige el color con que tu máquina aparece en las demás, compartido con el resto de los ajustes. | Lo siguiente |
| **Punteros fantasma** | Con una tecla, tu mouse mueve un puntero de tu color en otra pantalla, mientras su dueño conserva el suyo. Señala; nunca hace clic. | Diseñado: un tipo de mensaje nuevo, que un Beamer normal ignora |
| **Presencia** | Ver de un vistazo quién está enlazado, quién maneja qué y dónde está el puntero de cada uno. | Idea |
| **Archivos** | Enviar un archivo a la máquina en la que estás, por el mismo enlace cifrado. | Idea, del primer prototipo de FLOW |

Los mensajes nuevos van por el mismo enlace cifrado y se anuncian como capacidades, y el protocolo dice que un peer ignora los tipos de mensaje que no conoce. Así una máquina con FLOW puede seguir emparejándose y cruzando con un Beamer 1.5.x normal mientras aprende cosas nuevas.

### `// 05` Cómo funciona

| Pieza | Cómo | Dónde |
|---|---|---|
| **Captura** | Un event tap de Quartz en el Mac. Hooks de bajo nivel más Raw Input en Windows, para que el empuje se mida incluso justo en el borde. libei y el portal InputCapture en Wayland. Los eventos inyectados se marcan para que nunca vuelvan a capturarse. | [`mac_app/bridge.py`](mac_app/bridge.py), [`win_app/capture_win.py`](win_app/capture_win.py) |
| **Cruce** | Pantallas, zonas y el modelo de presión, iguales en todas las plataformas, como funciones puras del puntero, los deltas y un reloj. | [`core/return_edge.py`](core/return_edge.py), [`core/ways.py`](core/ways.py) |
| **Dueño del input** | Un solo dueño por máquina a la vez, un número de ruta para que una petición tardía nunca mueva un input que ya se movió, y todas las teclas presionadas se sueltan cuando termina. | [`core/owner.py`](core/owner.py), [WIRE.md §4–5](WIRE.md) |
| **El enlace** | TCP entre peers. X25519 por enlace, HKDF-SHA-256 sobre el token de emparejamiento, tramas ChaCha20-Poly1305 con contadores estrictos, una clave por dirección. Con secreto hacia adelante y sin servidor. | [`core/protocol.py`](core/protocol.py), [WIRE.md §2](WIRE.md) |
| **Emparejamiento** | CPace (X25519, SHA-512) con un código de seis dígitos que muestra una máquina y escribe la otra; da a cada par de máquinas su propio token. | [`core/pairing.py`](core/pairing.py), [PAIRING.md](PAIRING.md) |
| **Seguridad** | Si un enlace se cae o un acuse de recibo se atrasa, el input vuelve solo a casa. Un hook nunca se bloquea. | [`core/link.py`](core/link.py) |
| **Etiqueta de visitante** | Quién es el dueño de esta máquina, convertido en una etiqueta que sigue al puntero. | [`core/visitor.py`](core/visitor.py), [`mac_app/visitor_badge.py`](mac_app/visitor_badge.py), [`win_app/visitor_badge_win.py`](win_app/visitor_badge_win.py) |

El protocolo está documentado con el detalle suficiente para escribir otra implementación: [WIRE.md](WIRE.md), [PAIRING.md](PAIRING.md) y [SECURITY.md](SECURITY.md).

### `// 06` Compilar desde el código

FLOW todavía no tiene versiones publicadas, y la app sigue instalándose con el nombre de Beamer mientras avanza el cambio de nombre. Ambas compilaciones necesitan Python 3.

**Mac.** Haz doble clic en `mac_app/Install-Beamer-Mac.command`. Compila una app autónoma y la instala en `/Applications`. Ábrela, concede Accesibilidad y Monitoreo de entrada en la página Permissions y reiníciala desde esa misma página.

**Windows.** Desde un PowerShell de administrador:

```powershell
win_app\Install-Beamer.ps1
```

Instala la app, un acceso directo en el menú Inicio y las dos reglas del firewall.

**Tests.** Unos 3.300 del lado del Mac y 3.400 del lado de Windows, con la suite compartida de `core/` corriendo dentro de cada uno.

```sh
cd mac_app && .venv/bin/python3 -m unittest discover -s tests -t ..
```

```powershell
cd win_app; python -m unittest discover -s tests
```

Todo sobre instalación, Linux, ajustes, límites conocidos y solución de problemas está en la guía original de Beamer, completa en [docs/BEAMER.md](docs/BEAMER.md) (en inglés).

### `// 07` Construido sobre Beamer

FLOW es un fork de **[Beamer](https://github.com/kalkman-code/beamer)** 1.5.8, escrito y mantenido por **Toby George Read Kalkman** ([Kalkman Code](https://kalkmancode.co.uk/beamer)). Beamer es software libre bajo GPL-3.0, y casi todo lo que hay en este repositorio es trabajo suyo: la captura de entrada en tres plataformas, el motor de cruce y sus efectos, el emparejamiento, el enlace cifrado, las apps de ajustes y la documentación.

Lo que FLOW agrega o cambia está en la historia de este repositorio, a partir del commit `Beamer 1.5.8`. Si solo necesitas un muy buen KVM por software, usa Beamer: tiene versiones firmadas para Mac, Windows y Linux. Los reportes de errores sobre lo que hace Beamer van al repositorio original, y la idea es que FLOW siga las versiones de Beamer.

El FLOW anterior, el prototipo en Rust con descubrimiento mDNS y envío de archivos, sigue en [4mser/flow-legacy](https://github.com/4mser/flow-legacy).

### `// 08` Licencia

GPL-3.0-only, igual que Beamer. Ver [LICENSE](LICENSE). Los derechos de Beamer siguen siendo de Toby Kalkman; los cambios de FLOW son © 2026 Nicolás Moreno, bajo la misma licencia.

Las animaciones de este README son ilustraciones renderizadas cuadro a cuadro desde HTML, no grabaciones de pantalla. Los íconos son de [Lucide](https://lucide.dev) (ISC); las tipografías son Space Grotesk e IBM Plex Mono (SIL OFL 1.1).

<p align="center">
  <img src="assets/flow/footer.svg" width="100%" alt="FLOW, construido sobre Beamer, GPL-3.0">
</p>
