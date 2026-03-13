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
echo Run dist\house_puzzle\house_puzzle.exe to start.
echo.
pause
