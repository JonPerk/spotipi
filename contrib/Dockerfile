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

FROM debian:bookworm

RUN echo "deb http://deb.debian.org/debian bookworm main" > /etc/apt/sources.list && \
	echo "deb http://deb.debian.org/debian bookworm-updates main" >> /etc/apt/sources.list && \
	echo "deb http://deb.debian.org/debian-security bookworm-security main" >> /etc/apt/sources.list

RUN dpkg --add-architecture arm64 && \
	dpkg --add-architecture armhf && \
	dpkg --add-architecture armel && \
	# apt-get update && \
	# apt-get install -y lsb_release && \
	# apt-get clean all && \
	# apt-get build-dep . && \
	# sed "s/#DIST#/$(lsb_release -cs)/g" debian/changelog.in > debian/changelog && \
	# dpkg-buildpackage --no-sign && \
	apt-get update && \
	apt-get install -y --reinstall \
	build-essential \
	cmake \
	crossbuild-essential-arm64 \
	crossbuild-essential-armel \
	crossbuild-essential-armhf \
	curl \
	git \
	libasound2-dev \
	libasound2-dev:arm64 \
	libasound2-dev:armel \
	libasound2-dev:armhf \
	libclang-dev \
	libpulse0 \
	libpulse0:arm64 \
	libpulse0:armel \
	libpulse0:armhf \
	# cec
	libudev-dev \
	libxrandr-dev \
	pkg-config \
	python3-dev \
	# cec some might not be necessary
	libp8-platform-dev \
	dh-python \
	x11proto-core-dev \
	libncurses-dev \
	pkg-kde-tools \
	libudev-dev \
	libp8-platform-dev \
	gcc \
	libc6 \
	libc6-dev \
	zlib1g-dev \
	lib32-zlib \
	
	gcc-arm-linux-gnueabihf \
	# shared keep this
	swig && \
	apt-get upgrade

# Install Pi tools for armv6.
RUN mkdir /pi && \
    git -C /pi clone --depth=1 https://github.com/raspberrypi/tools.git

RUN git clone https://github.com/Pulse-Eight/platform.git && \
	mkdir platform/build && \
	cd platform/build && \
	cmake .. && \
	make && \
	make install && \
	cd && \ 
	git clone https://github.com/Pulse-Eight/libcec.git && \
	mkdir libcec/build && \
	cd libcec/build && \
	cmake -DCMAKE_TOOLCHAIN_FILE=../cmake/CrossCompile.cmake \
      -DXCOMPILE_BASE_PATH=/pi/tools/arm-bcm2708/arm-bcm2708hardfp-linux-gnueabi \
      -DXCOMPILE_LIB_PATH=/opt/vc/lib \
      -DRPI_INCLUDE_DIR=/opt/vc/include \
      -DRPI_LIB_DIR=/opt/vc/lib \
      .. && \
	make -j4 && \
	make install && \
	ldconfig && \
	cd

ENV PATH="/root/.cargo/bin/:${PATH}"
#RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain 1.81 -y && \
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	rustup target add aarch64-unknown-linux-gnu && \
	rustup target add arm-unknown-linux-gnueabi && \
	rustup target add arm-unknown-linux-gnueabihf && \
	cargo install bindgen-cli && \
	mkdir /.cargo && \
	echo '[target.aarch64-unknown-linux-gnu]\nlinker = "aarch64-linux-gnu-gcc"' > /.cargo/config && \
	echo '[target.arm-unknown-linux-gnueabihf]\nlinker = "arm-linux-gnueabihf-gcc"' >> /.cargo/config && \
	echo '[target.arm-unknown-linux-gnueabi]\nlinker = "arm-linux-gnueabi-gcc"' >> /.cargo/config

ENV CARGO_TARGET_DIR=/build
ENV CARGO_HOME=/build/cache
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV PATH="/pi/tools/arm-bcm2708/arm-linux-gnueabihf/bin:${PATH}"
ENV PKG_CONFIG_PATH_aarch64-unknown-linux-gnu=/usr/lib/aarch64-linux-gnu/pkgconfig/
ENV PKG_CONFIG_PATH_arm-unknown-linux-gnueabihf=/usr/lib/arm-linux-gnueabihf/pkgconfig/
ENV PKG_CONFIG_PATH_arm-unknown-linux-gnueabi=/usr/lib/arm-linux-gnueabi/pkgconfig/

ADD . /src
WORKDIR /src
CMD ["/src/contrib/docker-build.sh"]
