# Sneuit

Sneuit ("sneeuw" in het Nijmeegs) is een kleine macOS-app die sneeuw laat vallen over je scherm. De sneeuw blijft liggen op je vensters en hoopt zich onderaan het scherm op, met zwaartekracht: sleep je een venster, dan glijdt de sneeuw eraf. In de menubalk zit een menu met o.a. een sneeuwstorm, een sneeuwpop en een jongetje dat sneeuwballen gooit.

Geschreven in Rust (objc2), zonder Xcode.

## Downloaden

[Sneuit.zip](https://timvanbergen.nl/download/Sneuit.zip) — macOS 12 of nieuwer, Apple silicon. De app is niet door Apple genotariseerd: open hem de eerste keer via Systeeminstellingen › Privacy en beveiliging › Toch openen.

## Zelf bouwen

```sh
./bundle.sh     # bouwt target/Sneuit.app
./install.sh    # zet hem in /Applications en start hem bij het inloggen
./uninstall.sh  # haalt alles weer weg
```

Je hebt Rust nodig (`rustup`) en de Xcode Command Line Tools.
