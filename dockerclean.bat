SETLOCAL ENABLEDELAYEDEXPANSION
SET ants=
FOR /F "tokens=* USEBACKQ" %%F IN (`docker container ls --all --filter^=ancestor^=%1 --format "{{.ID}}"`) DO (
  call set "ants=%%ants%% %%F"
)
docker rm -f %ants%
docker rmi %1
ENDLOCAL