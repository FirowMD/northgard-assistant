@echo off
REM Run PyInstaller with icon
pyinstaller --noconfirm --onefile --name=buildmaker --windowed --icon=app_icon.ico --add-data "app_icon.ico;." --add-data "res;res" --add-data "clan_lores.json;." main.py

REM Wait a moment to ensure PyInstaller has finished
timeout /t 2 /nobreak

REM Copy resources to dist folder
xcopy /E /I /Y res dist\res
copy /Y app_icon.ico dist\
copy /Y clan_lores.json dist\

echo Build complete! Files have been copied to the dist folder.
pause
