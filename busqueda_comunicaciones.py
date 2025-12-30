from playwright.sync_api import sync_playwright
from dotenv import load_dotenv
import pandas as pd
from tkinter import filedialog
import os
from time import sleep


load_dotenv()


def obtener_comunicaciones() -> list[str]:
    columnas = ["CCOO N°", "ORGANISMO"]
    archivo = filedialog.askopenfilename(filetypes=[("Archivos Excel", "*.xlsx")])
    df = pd.read_excel(archivo, sheet_name=0, usecols=columnas)
    df = df[df["ORGANISMO"].isnull()]
    df = df.drop(columns=["ORGANISMO"])
    return df["CCOO N°"].tolist()


def buscar_comunicaciones(comunicaciones: list[str]):
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=False)
        context = browser.new_context(accept_downloads=True)
        page = context.new_page()

        # Configurar timeout por defecto
        page.set_default_timeout(30000)  # 30 segundos

        page.goto("http://euc.gcba.gob.ar/ccoo-web/")
        page.wait_for_load_state("networkidle")

        # Login
        boxes = page.locator(".form-control.z-textbox")
        boxes.first.wait_for()

        user = os.getenv("SADE_USER_CECILIA")
        password = os.getenv("SADE_PASSWORD_CECILIA")
        if user is None or password is None:
            raise ValueError("Environment variables for SADE credentials not found")

        boxes.nth(0).fill(user)
        boxes.nth(1).fill(password)
        page.locator(".btn.btn-default.z-button").click()
        page.wait_for_load_state("networkidle")
        sleep(2)

        for comunicacion in comunicaciones:
            page.locator(".z-textbox").nth(0).fill(comunicacion)
            page.locator(".z-button").nth(2).click()
            page.wait_for_load_state("networkidle")
            sleep(1)
            page.locator(".boton-sin-caja.z-button").nth(29).click()
            page.wait_for_load_state("networkidle")
            sleep(1)
            descargas_loc = page.locator(".z-icon-download")
            try:
                with page.expect_download(timeout=30000) as download_info:
                    descargas_loc.nth(0).click()
                download = download_info.value
                save_path = os.path.join(
                    os.path.expanduser("~"),
                    "Downloads",
                    download.suggested_filename,
                )
                download.save_as(save_path)
                sleep(0.5)
            except Exception as e:
                print(f"Error descargando archivo de comunicación {e}")
                continue
            # Volver a la lista de comunicaciones
            page.locator(".btn.z-button").nth(0).click()
            sleep(1)
            page.wait_for_load_state("networkidle")


def main():
    comunicaciones: list[str] = obtener_comunicaciones()
    buscar_comunicaciones(comunicaciones)


if __name__ == "__main__":
    main()
