from pathlib import Path

path = Path("src/app.rs")
text = path.read_text(encoding="utf-8")
start = text.find("        page-palette := Rectangle {")
end = text.find("#[derive(Clone, Copy)]\nstruct Rgb", start)
if start < 0 or end < 0:
    raise SystemExit("No se encontró el bloque page-palette")

block = r'''        page-palette := Rectangle {
            visible: root.editor-active && root.page-menu-open;
            x: island.x + island.width - 430px;
            y: island.y + island.height + 5px;
            width: 420px;
            height: 100px;
            border-radius: 9px;
            border-width: 1px;
            border-color: #354052;
            background: rgba(10, 13, 20, 0.98);

            Text {
                x: 10px; y: 5px; width: 50px; height: 24px;
                text: "PAPEL"; color: #7f8b9b; font-size: 10px; vertical-alignment: center;
            }
            Rectangle {
                x: 62px; y: 5px; width: 62px; height: 24px; border-radius: 5px;
                background: paper-carta.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "Carta"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                paper-carta := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("paper", "letter"); }
                }
            }
            Rectangle {
                x: 128px; y: 5px; width: 62px; height: 24px; border-radius: 5px;
                background: paper-oficio.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "Oficio"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                paper-oficio := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("paper", "oficio"); }
                }
            }
            Rectangle {
                x: 194px; y: 5px; width: 62px; height: 24px; border-radius: 5px;
                background: paper-legal.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "Legal"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                paper-legal := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("paper", "legal"); }
                }
            }
            Rectangle {
                x: 260px; y: 5px; width: 62px; height: 24px; border-radius: 5px;
                background: paper-a4.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "A4"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                paper-a4 := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("paper", "a4"); }
                }
            }
            Rectangle {
                x: 326px; y: 5px; width: 62px; height: 24px; border-radius: 5px;
                background: paper-a5.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "A5"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                paper-a5 := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("paper", "a5"); }
                }
            }

            Text {
                x: 10px; y: 35px; width: 50px; height: 24px;
                text: "ORIENT."; color: #7f8b9b; font-size: 10px; vertical-alignment: center;
            }
            Rectangle {
                x: 62px; y: 35px; width: 92px; height: 24px; border-radius: 5px;
                background: portrait-touch.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "Vertical"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                portrait-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("orientation", "portrait"); }
                }
            }
            Rectangle {
                x: 158px; y: 35px; width: 100px; height: 24px; border-radius: 5px;
                background: landscape-touch.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "Horizontal"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                landscape-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("orientation", "landscape"); }
                }
            }
            Text {
                x: 268px; y: 35px; width: 140px; height: 24px;
                text: root.page-orientation-text; color: #b8bb26; font-size: 9px; vertical-alignment: center;
            }

            Text {
                x: 10px; y: 65px; width: 50px; height: 24px;
                text: "MARGEN"; color: #7f8b9b; font-size: 10px; vertical-alignment: center;
            }
            Rectangle {
                x: 62px; y: 65px; width: 76px; height: 24px; border-radius: 5px;
                background: margin-left-touch.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "I " + root.margin-left-text + " mm"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                margin-left-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("margin-left", "cycle"); }
                }
            }
            Rectangle {
                x: 142px; y: 65px; width: 76px; height: 24px; border-radius: 5px;
                background: margin-right-touch.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "D " + root.margin-right-text + " mm"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                margin-right-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("margin-right", "cycle"); }
                }
            }
            Rectangle {
                x: 222px; y: 65px; width: 76px; height: 24px; border-radius: 5px;
                background: margin-top-touch.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "S " + root.margin-top-text + " mm"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                margin-top-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("margin-top", "cycle"); }
                }
            }
            Rectangle {
                x: 302px; y: 65px; width: 76px; height: 24px; border-radius: 5px;
                background: margin-bottom-touch.has-hover ? #172334 : transparent;
                Text { width: 100%; height: 100%; text: "B " + root.margin-bottom-text + " mm"; color: #dfe8ef; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                margin-bottom-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.page-action("margin-bottom", "cycle"); }
                }
            }
        }
    }
}

'''
text = text[:start] + block + text[end:]
path.write_text(text, encoding="utf-8")
print("Bloque de página corregido")
