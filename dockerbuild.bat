SETLOCAL ENABLEDELAYEDEXPANSION
SET ants=
FOR /F "tokens=* USEBACKQ" %%F IN (`docker container ls --all --filter^=ancestor^=spotipi-cross --format "{{.ID}}"`) DO (
  SET ants=%ants% %%F
)
docker rm -f %ants%
docker rmi spotipi-cross
docker build -t spotipi-cross -f contrib/Dockerfile .
docker run -v spotipi-target:/build spotipi-cross cargo build --release --target aarch64-unknown-linux-gnu --no-default-features --features "alsa-backend pulseaudio-backend with-avahi"
set loc=%~dp0
set loc=%loc:\=/%
set loc=%loc:~0,-1%
docker run --rm -v spotipi-target:/src -v %loc%:/dest alpine sh -c "cp /src/aarch64-unknown-linux-gnu/release/spotipi /dest/target/aarch64-unknown-linux-gnu/release"
ENDLOCAL