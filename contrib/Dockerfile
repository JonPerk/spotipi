# Cross compilation environment for spotipi
# Build the docker image from the root of the project with the following command :
# $ docker build -t spotipi-cross -f contrib/Dockerfile .
#
# The resulting image can be used to build spotipi for linux x86_64, armhf, armel, aarch64
# $ docker run -v /tmp/spotipi-build:/build spotipi-cross
#
# The compiled binaries will be located in /tmp/spotipi-build
#
# If only one architecture is desired, cargo can be invoked directly with the appropriate options :
# $ docker run -v /tmp/spotipi-build:/build spotipi-cross cargo build --release --no-default-features --features "alsa-backend with-libmdns"
# $ docker run -v /tmp/spotipi-build:/build spotipi-cross cargo build --release --target arm-unknown-linux-gnueabihf --no-default-features --features "alsa-backend with-libmdns"
# $ docker run -v /tmp/spotipi-build:/build spotipi-cross cargo build --release --target arm-unknown-linux-gnueabi --no-default-features --features "alsa-backend with-libmdns"
# $ docker run -v /tmp/spotipi-build:/build spotipi-cross cargo build --release --target aarch64-unknown-linux-gnu --no-default-features --features "alsa-backend with-libmdns"
# NOTE: Currently only one architecture can be built at a time. Installing multiple copies of libp8-platform-dev at once is not working
FROM debian:bookworm

RUN echo "deb http://deb.debian.org/debian bookworm main" > /etc/apt/sources.list && \
	echo "deb http://deb.debian.org/debian bookworm-updates main" >> /etc/apt/sources.list && \
	echo "deb http://deb.debian.org/debian-security bookworm-security main" >> /etc/apt/sources.list

RUN dpkg --add-architecture arm64 && \
	# dpkg --add-architecture armhf && \
	# dpkg --add-architecture armel && \
	apt update && \
	apt-get update && \
	apt-get install -y \
	build-essential \
	cmake \
	crossbuild-essential-arm64 \
	# crossbuild-essential-armel \
	# crossbuild-essential-armhf \
	curl \
	dh-python \
	git \
	libasound2-dev \
	libasound2-dev:arm64 \
	# libasound2-dev:armel \
	# libasound2-dev:armhf \
	libclang-dev \
	libncurses-dev \
	libncurses-dev:arm64 \
	# libncurses-dev:armel \
	# libncurses-dev:armhf \
	libpulse0 \
	libpulse0:arm64 \
	# libpulse0:armel \
	# libpulse0:armhf \
	libp8-platform-dev:arm64 \
	# libp8-platform-dev:armel \
	# libp8-platform-dev:armhf \
	libudev-dev \
	libudev-dev:arm64 \
	# libudev-dev:armel \
	# libudev-dev:armhf \
	libxrandr-dev \
	libxrandr-dev:arm64 \
	# libxrandr-dev:armel \
	# libxrandr-dev:armhf \
	pkg-config \
	pkg-config:arm64 \
	# pkg-config:armel \
	# pkg-config:armhf \
	python3-dev \
	# python3-dev:armel \
	# python3-dev:armhf \
	pkg-kde-tools \
	swig \
	x11proto-core-dev && \
	apt-get upgrade

ENV PATH="/root/.cargo/bin/:${PATH}"
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	rustup target add aarch64-unknown-linux-gnu && \
	# rustup target add arm-unknown-linux-gnueabi && \
	# rustup target add arm-unknown-linux-gnueabihf && \
	cargo install bindgen-cli && \
	mkdir /.cargo && \
	echo '[target.aarch64-unknown-linux-gnu]\nlinker = "aarch64-linux-gnu-gcc"\nrustflags = ["-C", "link-args=-Wl,-rpath=/usr/lib/aarch64-linux-gnu"]' > /.cargo/config
	# echo '[target.arm-unknown-linux-gnueabihf]\nlinker = "arm-linux-gnueabihf-gcc"\nrustflags = ["-C", "link-args=-Wl,-rpath=/usr/lib/arm-linux-gnueabihf"]' >> /.cargo/config && \
	# echo '[target.arm-unknown-linux-gnueabi]\nlinker = "arm-linux-gnueabi-gcc"\nrustflags = ["-C", "link-args=-Wl,-rpath=/usr/lib/arm-linux-gnueabi"]' >> /.cargo/config

ENV CARGO_TARGET_DIR=/build
ENV CARGO_HOME=/build/cache
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV PKG_CONFIG_PATH_aarch64-unknown-linux-gnu=/usr/lib/aarch64-linux-gnu/pkgconfig/
# ENV PKG_CONFIG_PATH_arm-unknown-linux-gnueabihf=/usr/lib/arm-linux-gnueabihf/pkgconfig/
# ENV PKG_CONFIG_PATH_arm-unknown-linux-gnueabi=/usr/lib/arm-linux-gnueabi/pkgconfig/

RUN --mount=type=bind,source=CrossCompile.cmake,target=/tmp/CrossCompile.cmake \
	git clone https://github.com/Pulse-Eight/platform.git && \
	mkdir platform/build && \
	cd platform/build && \
	cmake -DCMAKE_TOOLCHAIN_FILE=/tmp/CrossCompile.cmake \
	-DXCOMPILE_BASE_PATH=/ \
	-DXCOMPILE_LIB_PATH=/lib/aarch64-linux-gnu \
	.. && \
	make -j4 && \
	make install && \
	cd && \ 
	git clone https://github.com/Pulse-Eight/libcec.git && \
	mkdir libcec/build && \
	# latest is broken so revert to working commit a2b589a987e2b409e61b04126aadca7c441138bc 88846e801661cd68db146593197a503f676f2183 bf1359face011f554eb82404712d9520127ff5a3
	cd libcec && \
	git reset --hard 88846e801661cd68db146593197a503f676f2183 && \
	cd ../ && \
	# resume normal operations
	cd libcec/build && \
	cmake -DCMAKE_TOOLCHAIN_FILE=/tmp/CrossCompile.cmake \
	-DXCOMPILE_BASE_PATH=/ \
	-DXCOMPILE_LIB_PATH=/lib/aarch64-linux-gnu \
	-DHAVE_LINUX_API=1 \
	-DHAVE_RPI_API=0 \
	-DRPI_INCLUDE_DIR=/usr/lib \
	-DRPI_LIB_DIR=/usr/include \
	.. && \
	make -j4 && \
	make install && \
	ldconfig && \
	cd

ADD . /src
WORKDIR /src
CMD ["/src/contrib/docker-build.sh"]
