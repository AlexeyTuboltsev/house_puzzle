@echo off
echo === House Puzzle Editor — Windows Build ===
echo.
echo Installing dependencies...
pip install -r requirements.txt pyinstaller
echo.
echo Building executable...
pyinstaller house_puzzle.spec --noconfirm
echo.
echo Done! Output in dist\house_puzzle\
echo.
echo Prerequisites:
echo   - ImageMagick must be installed and on PATH
echo     Download: https://imagemagick.org/script/download.php#windows
echo.
pause
