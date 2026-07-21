# WAP-Port Buildfehler

Workflow-Exitcode: 101

```text
/tmp/wap-port.patch.gz.b64: OK
/tmp/wap-port.patch.gz: OK
/tmp/wap-port.patch: OK
>>> sudo apt-get update
Get:1 file:/etc/apt/apt-mirrors.txt Mirrorlist [144 B]
Hit:6 https://packages.microsoft.com/repos/azure-cli noble InRelease
Hit:2 http://azure.archive.ubuntu.com/ubuntu noble InRelease
Get:7 https://packages.microsoft.com/ubuntu/24.04/prod noble InRelease [3600 B]
Get:3 http://azure.archive.ubuntu.com/ubuntu noble-updates InRelease [126 kB]
Get:4 http://azure.archive.ubuntu.com/ubuntu noble-backports InRelease [126 kB]
Get:5 http://azure.archive.ubuntu.com/ubuntu noble-security InRelease [126 kB]
Get:8 https://dl.google.com/linux/chrome-stable/deb stable InRelease [1825 B]
Get:9 https://packages.microsoft.com/ubuntu/24.04/prod noble/main amd64 Packages [233 kB]
Get:10 https://packages.microsoft.com/ubuntu/24.04/prod noble/main armhf Packages [11.7 kB]
Get:11 https://packages.microsoft.com/ubuntu/24.04/prod noble/main arm64 Packages [200 kB]
Get:12 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 Packages [1122 kB]
Get:13 http://azure.archive.ubuntu.com/ubuntu noble-updates/main Translation-en [274 kB]
Get:14 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 Components [180 kB]
Get:15 http://azure.archive.ubuntu.com/ubuntu noble-updates/universe amd64 Packages [1666 kB]
Get:16 http://azure.archive.ubuntu.com/ubuntu noble-updates/universe Translation-en [329 kB]
Get:17 http://azure.archive.ubuntu.com/ubuntu noble-updates/universe amd64 Components [388 kB]
Get:18 http://azure.archive.ubuntu.com/ubuntu noble-updates/restricted amd64 Packages [1274 kB]
Get:19 http://azure.archive.ubuntu.com/ubuntu noble-updates/restricted Translation-en [291 kB]
Get:20 http://azure.archive.ubuntu.com/ubuntu noble-updates/multiverse amd64 Components [940 B]
Get:21 http://azure.archive.ubuntu.com/ubuntu noble-backports/main amd64 Components [5752 B]
Get:22 http://azure.archive.ubuntu.com/ubuntu noble-backports/universe amd64 Components [10.5 kB]
Get:23 http://azure.archive.ubuntu.com/ubuntu noble-security/main amd64 Packages [861 kB]
Get:24 http://azure.archive.ubuntu.com/ubuntu noble-security/main Translation-en [192 kB]
Get:25 http://azure.archive.ubuntu.com/ubuntu noble-security/main amd64 Components [46.3 kB]
Get:26 http://azure.archive.ubuntu.com/ubuntu noble-security/universe amd64 Packages [1180 kB]
Get:27 http://azure.archive.ubuntu.com/ubuntu noble-security/universe Translation-en [233 kB]
Get:28 http://azure.archive.ubuntu.com/ubuntu noble-security/universe amd64 Components [76.3 kB]
Get:29 http://azure.archive.ubuntu.com/ubuntu noble-security/restricted amd64 Packages [1178 kB]
Get:30 http://azure.archive.ubuntu.com/ubuntu noble-security/restricted Translation-en [272 kB]
Get:31 https://dl.google.com/linux/chrome-stable/deb stable/main amd64 Packages [1418 B]
Fetched 10.4 MB in 1s (8350 kB/s)
Reading package lists...
>>> sudo apt-get install --yes pkg-config libsoapysdr-dev
Reading package lists...
Building dependency tree...
Reading state information...
pkg-config is already the newest version (1.8.1-2build1).
The following additional packages will be installed:
  bladerf libairspy0 libasyncns0 libbladerf2 libboost-chrono1.83.0t64
  libboost-filesystem1.83.0 libboost-serialization1.83.0 libboost-thread1.83.0
  libflac12t64 libhackrf0 libhamlib4t64 libindi-data libindiclient1
  libjack-jackd2-0 liblimesuite23.11-1 libmirisdr4 libmp3lame0 libmpg123-0t64
  libnova-0.16-0t64 libopus0 libosmosdr0 libpulse0 librtaudio6 librtlsdr2
  libsamplerate0 libsndfile1 libsoapysdr0.8 libtecla1t64 libuhd4.6.0t64
  libvorbisenc2 limesuite-udev soapyosmo-common0.8 soapysdr0.8-module-airspy
  soapysdr0.8-module-all soapysdr0.8-module-audio soapysdr0.8-module-bladerf
  soapysdr0.8-module-hackrf soapysdr0.8-module-lms7 soapysdr0.8-module-mirisdr
  soapysdr0.8-module-osmosdr soapysdr0.8-module-redpitaya
  soapysdr0.8-module-remote soapysdr0.8-module-rfspace
  soapysdr0.8-module-rtlsdr soapysdr0.8-module-uhd
Suggested packages:
  bladerf-firmware bladerf-fpga jackd2 opus-tools pulseaudio libsoapysdr-doc
  uhd-host
The following NEW packages will be installed:
  bladerf libairspy0 libasyncns0 libbladerf2 libboost-chrono1.83.0t64
  libboost-filesystem1.83.0 libboost-serialization1.83.0 libboost-thread1.83.0
  libflac12t64 libhackrf0 libhamlib4t64 libindi-data libindiclient1
  libjack-jackd2-0 liblimesuite23.11-1 libmirisdr4 libmp3lame0 libmpg123-0t64
  libnova-0.16-0t64 libopus0 libosmosdr0 libpulse0 librtaudio6 librtlsdr2
  libsamplerate0 libsndfile1 libsoapysdr-dev libsoapysdr0.8 libtecla1t64
  libuhd4.6.0t64 libvorbisenc2 limesuite-udev soapyosmo-common0.8
  soapysdr0.8-module-airspy soapysdr0.8-module-all soapysdr0.8-module-audio
  soapysdr0.8-module-bladerf soapysdr0.8-module-hackrf soapysdr0.8-module-lms7
  soapysdr0.8-module-mirisdr soapysdr0.8-module-osmosdr
  soapysdr0.8-module-redpitaya soapysdr0.8-module-remote
  soapysdr0.8-module-rfspace soapysdr0.8-module-rtlsdr soapysdr0.8-module-uhd
0 upgraded, 46 newly installed, 0 to remove and 65 not upgraded.
Need to get 11.2 MB of archives.
After this operation, 48.3 MB of additional disk space will be used.
Get:1 file:/etc/apt/apt-mirrors.txt Mirrorlist [144 B]
Get:2 http://azure.archive.ubuntu.com/ubuntu noble/main amd64 libasyncns0 amd64 0.8-6build4 [11.3 kB]
Get:3 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libbladerf2 amd64 0.2023.02-4build1 [181 kB]
Get:4 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 libboost-chrono1.83.0t64 amd64 1.83.0-2.1ubuntu3.2 [245 kB]
Get:5 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 libboost-filesystem1.83.0 amd64 1.83.0-2.1ubuntu3.2 [284 kB]
Get:6 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 libboost-serialization1.83.0 amd64 1.83.0-2.1ubuntu3.2 [341 kB]
Get:7 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 libboost-thread1.83.0 amd64 1.83.0-2.1ubuntu3.2 [276 kB]
Get:8 http://azure.archive.ubuntu.com/ubuntu noble/main amd64 libflac12t64 amd64 1.4.3+ds-2.1ubuntu2 [197 kB]
Get:9 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libindi-data all 1.9.9+dfsg-3build3 [10.6 kB]
Get:10 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libnova-0.16-0t64 amd64 0.16-5.1build1 [953 kB]
Get:11 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libindiclient1 amd64 1.9.9+dfsg-3build3 [149 kB]
Get:12 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libhamlib4t64 amd64 4.5.5-3.2build2 [1054 kB]
Get:13 http://azure.archive.ubuntu.com/ubuntu noble/main amd64 libopus0 amd64 1.4-1build1 [208 kB]
Get:14 http://azure.archive.ubuntu.com/ubuntu noble/main amd64 libsamplerate0 amd64 0.2.2-4build1 [1344 kB]
Get:15 http://azure.archive.ubuntu.com/ubuntu noble/main amd64 libjack-jackd2-0 amd64 1.9.21~dfsg-3ubuntu3 [289 kB]
Get:16 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 liblimesuite23.11-1 amd64 23.11.0+dfsg-2build2 [258 kB]
Get:17 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libmirisdr4 amd64 2.0.0-4 [20.2 kB]
Get:18 http://azure.archive.ubuntu.com/ubuntu noble/main amd64 libmp3lame0 amd64 3.100-6build1 [142 kB]
Get:19 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 libmpg123-0t64 amd64 1.32.5-1ubuntu1.1 [169 kB]
Get:20 http://azure.archive.ubuntu.com/ubuntu noble/main amd64 libvorbisenc2 amd64 1.3.7-1build3 [80.8 kB]
Get:21 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 libsndfile1 amd64 1.2.2-1ubuntu5.24.04.1 [209 kB]
Get:22 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 libpulse0 amd64 1:16.1+dfsg1-2ubuntu10.1 [292 kB]
Get:23 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 librtaudio6 amd64 5.2.0~ds1-2build3 [50.7 kB]
Get:24 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 librtlsdr2 amd64 2.0.1-2build1 [31.5 kB]
Get:25 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libsoapysdr0.8 amd64 0.8.1-4build1 [107 kB]
Get:26 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libsoapysdr-dev amd64 0.8.1-4build1 [29.3 kB]
Get:27 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libtecla1t64 amd64 1.6.3-3.1build1 [69.8 kB]
Get:28 http://azure.archive.ubuntu.com/ubuntu noble-updates/universe amd64 libuhd4.6.0t64 amd64 4.6.0.0+ds1-5.1ubuntu0.24.04.1 [3414 kB]
Get:29 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 limesuite-udev all 23.11.0+dfsg-2build2 [5452 B]
Get:30 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapyosmo-common0.8 amd64 0.2.5-8build3 [19.6 kB]
Get:31 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libairspy0 amd64 1.0.10-3build1 [21.4 kB]
Get:32 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-airspy amd64 0.2.0-4 [27.1 kB]
Get:33 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-audio amd64 0.1.1-5build2 [36.9 kB]
Get:34 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-bladerf amd64 0.4.1-5 [51.2 kB]
Get:35 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libhackrf0 amd64 2023.01.1-9build1 [18.8 kB]
Get:36 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-hackrf amd64 0.3.4-1 [36.8 kB]
Get:37 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-lms7 amd64 23.11.0+dfsg-2build2 [49.4 kB]
Get:38 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-mirisdr amd64 0.2.5-8build3 [47.2 kB]
Get:39 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 libosmosdr0 amd64 0.1.8.effcaa7-10build1 [11.1 kB]
Get:40 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-osmosdr amd64 0.2.5-8build3 [24.9 kB]
Get:41 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-redpitaya amd64 0.1.1-5 [16.9 kB]
Get:42 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-remote amd64 0.5.2-4 [116 kB]
Get:43 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-rfspace amd64 0.2.5-8build3 [81.9 kB]
Get:44 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-rtlsdr amd64 0.3.3-1build1 [35.3 kB]
Get:45 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-uhd amd64 0.4.1-4build4 [63.8 kB]
Get:46 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 soapysdr0.8-module-all amd64 0.8.1-4build1 [4096 B]
Get:47 http://azure.archive.ubuntu.com/ubuntu noble/universe amd64 bladerf amd64 0.2023.02-4build1 [128 kB]
Fetched 11.2 MB in 4s (2924 kB/s)
Selecting previously unselected package libasyncns0:amd64.
(Reading database ... (Reading database ... 5%(Reading database ... 10%(Reading database ... 15%(Reading database ... 20%(Reading database ... 25%(Reading database ... 30%(Reading database ... 35%(Reading database ... 40%(Reading database ... 45%(Reading database ... 50%(Reading database ... 55%(Reading database ... 60%(Reading database ... 65%(Reading database ... 70%(Reading database ... 75%(Reading database ... 80%(Reading database ... 85%(Reading database ... 90%(Reading database ... 95%(Reading database ... 100%(Reading database ... 202507 files and directories currently installed.)
Preparing to unpack .../00-libasyncns0_0.8-6build4_amd64.deb ...
Unpacking libasyncns0:amd64 (0.8-6build4) ...
Selecting previously unselected package libbladerf2:amd64.
Preparing to unpack .../01-libbladerf2_0.2023.02-4build1_amd64.deb ...
Unpacking libbladerf2:amd64 (0.2023.02-4build1) ...
Selecting previously unselected package libboost-chrono1.83.0t64:amd64.
Preparing to unpack .../02-libboost-chrono1.83.0t64_1.83.0-2.1ubuntu3.2_amd64.deb ...
Unpacking libboost-chrono1.83.0t64:amd64 (1.83.0-2.1ubuntu3.2) ...
Selecting previously unselected package libboost-filesystem1.83.0:amd64.
Preparing to unpack .../03-libboost-filesystem1.83.0_1.83.0-2.1ubuntu3.2_amd64.deb ...
Unpacking libboost-filesystem1.83.0:amd64 (1.83.0-2.1ubuntu3.2) ...
Selecting previously unselected package libboost-serialization1.83.0:amd64.
Preparing to unpack .../04-libboost-serialization1.83.0_1.83.0-2.1ubuntu3.2_amd64.deb ...
Unpacking libboost-serialization1.83.0:amd64 (1.83.0-2.1ubuntu3.2) ...
Selecting previously unselected package libboost-thread1.83.0:amd64.
Preparing to unpack .../05-libboost-thread1.83.0_1.83.0-2.1ubuntu3.2_amd64.deb ...
Unpacking libboost-thread1.83.0:amd64 (1.83.0-2.1ubuntu3.2) ...
Selecting previously unselected package libflac12t64:amd64.
Preparing to unpack .../06-libflac12t64_1.4.3+ds-2.1ubuntu2_amd64.deb ...
Unpacking libflac12t64:amd64 (1.4.3+ds-2.1ubuntu2) ...
Selecting previously unselected package libindi-data.
Preparing to unpack .../07-libindi-data_1.9.9+dfsg-3build3_all.deb ...
Unpacking libindi-data (1.9.9+dfsg-3build3) ...
Selecting previously unselected package libnova-0.16-0t64:amd64.
Preparing to unpack .../08-libnova-0.16-0t64_0.16-5.1build1_amd64.deb ...
Unpacking libnova-0.16-0t64:amd64 (0.16-5.1build1) ...
Selecting previously unselected package libindiclient1:amd64.
Preparing to unpack .../09-libindiclient1_1.9.9+dfsg-3build3_amd64.deb ...
Unpacking libindiclient1:amd64 (1.9.9+dfsg-3build3) ...
Selecting previously unselected package libhamlib4t64:amd64.
Preparing to unpack .../10-libhamlib4t64_4.5.5-3.2build2_amd64.deb ...
Adding 'diversion of /lib/udev/rules.d/60-libhamlib4.rules to /lib/udev/rules.d/60-libhamlib4.rules.usr-is-merged by usr-is-merged'
Unpacking libhamlib4t64:amd64 (4.5.5-3.2build2) ...
Selecting previously unselected package libopus0:amd64.
Preparing to unpack .../11-libopus0_1.4-1build1_amd64.deb ...
Unpacking libopus0:amd64 (1.4-1build1) ...
Selecting previously unselected package libsamplerate0:amd64.
Preparing to unpack .../12-libsamplerate0_0.2.2-4build1_amd64.deb ...
Unpacking libsamplerate0:amd64 (0.2.2-4build1) ...
Selecting previously unselected package libjack-jackd2-0:amd64.
Preparing to unpack .../13-libjack-jackd2-0_1.9.21~dfsg-3ubuntu3_amd64.deb ...
Unpacking libjack-jackd2-0:amd64 (1.9.21~dfsg-3ubuntu3) ...
Selecting previously unselected package liblimesuite23.11-1:amd64.
Preparing to unpack .../14-liblimesuite23.11-1_23.11.0+dfsg-2build2_amd64.deb ...
Unpacking liblimesuite23.11-1:amd64 (23.11.0+dfsg-2build2) ...
Selecting previously unselected package libmirisdr4:amd64.
Preparing to unpack .../15-libmirisdr4_2.0.0-4_amd64.deb ...
Unpacking libmirisdr4:amd64 (2.0.0-4) ...
Selecting previously unselected package libmp3lame0:amd64.
Preparing to unpack .../16-libmp3lame0_3.100-6build1_amd64.deb ...
Unpacking libmp3lame0:amd64 (3.100-6build1) ...
Selecting previously unselected package libmpg123-0t64:amd64.
Preparing to unpack .../17-libmpg123-0t64_1.32.5-1ubuntu1.1_amd64.deb ...
Unpacking libmpg123-0t64:amd64 (1.32.5-1ubuntu1.1) ...
Selecting previously unselected package libvorbisenc2:amd64.
Preparing to unpack .../18-libvorbisenc2_1.3.7-1build3_amd64.deb ...
Unpacking libvorbisenc2:amd64 (1.3.7-1build3) ...
Selecting previously unselected package libsndfile1:amd64.
Preparing to unpack .../19-libsndfile1_1.2.2-1ubuntu5.24.04.1_amd64.deb ...
Unpacking libsndfile1:amd64 (1.2.2-1ubuntu5.24.04.1) ...
Selecting previously unselected package libpulse0:amd64.
Preparing to unpack .../20-libpulse0_1%3a16.1+dfsg1-2ubuntu10.1_amd64.deb ...
Unpacking libpulse0:amd64 (1:16.1+dfsg1-2ubuntu10.1) ...
Selecting previously unselected package librtaudio6:amd64.
Preparing to unpack .../21-librtaudio6_5.2.0~ds1-2build3_amd64.deb ...
Unpacking librtaudio6:amd64 (5.2.0~ds1-2build3) ...
Selecting previously unselected package librtlsdr2:amd64.
Preparing to unpack .../22-librtlsdr2_2.0.1-2build1_amd64.deb ...
Unpacking librtlsdr2:amd64 (2.0.1-2build1) ...
Selecting previously unselected package libsoapysdr0.8:amd64.
Preparing to unpack .../23-libsoapysdr0.8_0.8.1-4build1_amd64.deb ...
Unpacking libsoapysdr0.8:amd64 (0.8.1-4build1) ...
Selecting previously unselected package libsoapysdr-dev:amd64.
Preparing to unpack .../24-libsoapysdr-dev_0.8.1-4build1_amd64.deb ...
Unpacking libsoapysdr-dev:amd64 (0.8.1-4build1) ...
Selecting previously unselected package libtecla1t64:amd64.
Preparing to unpack .../25-libtecla1t64_1.6.3-3.1build1_amd64.deb ...
Unpacking libtecla1t64:amd64 (1.6.3-3.1build1) ...
Selecting previously unselected package libuhd4.6.0t64:amd64.
Preparing to unpack .../26-libuhd4.6.0t64_4.6.0.0+ds1-5.1ubuntu0.24.04.1_amd64.deb ...
Unpacking libuhd4.6.0t64:amd64 (4.6.0.0+ds1-5.1ubuntu0.24.04.1) ...
Selecting previously unselected package limesuite-udev.
Preparing to unpack .../27-limesuite-udev_23.11.0+dfsg-2build2_all.deb ...
Unpacking limesuite-udev (23.11.0+dfsg-2build2) ...
Selecting previously unselected package soapyosmo-common0.8:amd64.
Preparing to unpack .../28-soapyosmo-common0.8_0.2.5-8build3_amd64.deb ...
Unpacking soapyosmo-common0.8:amd64 (0.2.5-8build3) ...
Selecting previously unselected package libairspy0:amd64.
Preparing to unpack .../29-libairspy0_1.0.10-3build1_amd64.deb ...
Unpacking libairspy0:amd64 (1.0.10-3build1) ...
Selecting previously unselected package soapysdr0.8-module-airspy:amd64.
Preparing to unpack .../30-soapysdr0.8-module-airspy_0.2.0-4_amd64.deb ...
Unpacking soapysdr0.8-module-airspy:amd64 (0.2.0-4) ...
Selecting previously unselected package soapysdr0.8-module-audio:amd64.
Preparing to unpack .../31-soapysdr0.8-module-audio_0.1.1-5build2_amd64.deb ...
Unpacking soapysdr0.8-module-audio:amd64 (0.1.1-5build2) ...
Selecting previously unselected package soapysdr0.8-module-bladerf:amd64.
Preparing to unpack .../32-soapysdr0.8-module-bladerf_0.4.1-5_amd64.deb ...
Unpacking soapysdr0.8-module-bladerf:amd64 (0.4.1-5) ...
Selecting previously unselected package libhackrf0:amd64.
Preparing to unpack .../33-libhackrf0_2023.01.1-9build1_amd64.deb ...
Unpacking libhackrf0:amd64 (2023.01.1-9build1) ...
Selecting previously unselected package soapysdr0.8-module-hackrf:amd64.
Preparing to unpack .../34-soapysdr0.8-module-hackrf_0.3.4-1_amd64.deb ...
Unpacking soapysdr0.8-module-hackrf:amd64 (0.3.4-1) ...
Selecting previously unselected package soapysdr0.8-module-lms7:amd64.
Preparing to unpack .../35-soapysdr0.8-module-lms7_23.11.0+dfsg-2build2_amd64.deb ...
Unpacking soapysdr0.8-module-lms7:amd64 (23.11.0+dfsg-2build2) ...
Selecting previously unselected package soapysdr0.8-module-mirisdr:amd64.
Preparing to unpack .../36-soapysdr0.8-module-mirisdr_0.2.5-8build3_amd64.deb ...
Unpacking soapysdr0.8-module-mirisdr:amd64 (0.2.5-8build3) ...
Selecting previously unselected package libosmosdr0:amd64.
Preparing to unpack .../37-libosmosdr0_0.1.8.effcaa7-10build1_amd64.deb ...
Unpacking libosmosdr0:amd64 (0.1.8.effcaa7-10build1) ...
Selecting previously unselected package soapysdr0.8-module-osmosdr:amd64.
Preparing to unpack .../38-soapysdr0.8-module-osmosdr_0.2.5-8build3_amd64.deb ...
Unpacking soapysdr0.8-module-osmosdr:amd64 (0.2.5-8build3) ...
Selecting previously unselected package soapysdr0.8-module-redpitaya:amd64.
Preparing to unpack .../39-soapysdr0.8-module-redpitaya_0.1.1-5_amd64.deb ...
Unpacking soapysdr0.8-module-redpitaya:amd64 (0.1.1-5) ...
Selecting previously unselected package soapysdr0.8-module-remote:amd64.
Preparing to unpack .../40-soapysdr0.8-module-remote_0.5.2-4_amd64.deb ...
Unpacking soapysdr0.8-module-remote:amd64 (0.5.2-4) ...
Selecting previously unselected package soapysdr0.8-module-rfspace:amd64.
Preparing to unpack .../41-soapysdr0.8-module-rfspace_0.2.5-8build3_amd64.deb ...
Unpacking soapysdr0.8-module-rfspace:amd64 (0.2.5-8build3) ...
Selecting previously unselected package soapysdr0.8-module-rtlsdr:amd64.
Preparing to unpack .../42-soapysdr0.8-module-rtlsdr_0.3.3-1build1_amd64.deb ...
Unpacking soapysdr0.8-module-rtlsdr:amd64 (0.3.3-1build1) ...
Selecting previously unselected package soapysdr0.8-module-uhd:amd64.
Preparing to unpack .../43-soapysdr0.8-module-uhd_0.4.1-4build4_amd64.deb ...
Unpacking soapysdr0.8-module-uhd:amd64 (0.4.1-4build4) ...
Selecting previously unselected package soapysdr0.8-module-all:amd64.
Preparing to unpack .../44-soapysdr0.8-module-all_0.8.1-4build1_amd64.deb ...
Unpacking soapysdr0.8-module-all:amd64 (0.8.1-4build1) ...
Selecting previously unselected package bladerf.
Preparing to unpack .../45-bladerf_0.2023.02-4build1_amd64.deb ...
Unpacking bladerf (0.2023.02-4build1) ...
Setting up libmirisdr4:amd64 (2.0.0-4) ...
No diversion 'diversion of /lib/udev/rules.d/60-libmirisdr4.rules to /lib/udev/rules.d/60-libmirisdr4.rules.usr-is-merged by usr-is-merged', none removed.
Setting up libairspy0:amd64 (1.0.10-3build1) ...
No diversion 'diversion of /lib/udev/rules.d/60-libairspy0.rules to /lib/udev/rules.d/60-libairspy0.rules.usr-is-merged by usr-is-merged', none removed.
Setting up libboost-thread1.83.0:amd64 (1.83.0-2.1ubuntu3.2) ...
Setting up libhackrf0:amd64 (2023.01.1-9build1) ...
No diversion 'diversion of /lib/udev/rules.d/60-libhackrf0.rules to /lib/udev/rules.d/60-libhackrf0.rules.usr-is-merged by usr-is-merged', none removed.
Setting up libmpg123-0t64:amd64 (1.32.5-1ubuntu1.1) ...
Setting up libboost-filesystem1.83.0:amd64 (1.83.0-2.1ubuntu3.2) ...
Setting up limesuite-udev (23.11.0+dfsg-2build2) ...
Setting up libbladerf2:amd64 (0.2023.02-4build1) ...
No diversion 'diversion of /lib/udev/rules.d/88-nuand-bladerf1.rules to /lib/udev/rules.d/88-nuand-bladerf1.rules.usr-is-merged by usr-is-merged', none removed.
No diversion 'diversion of /lib/udev/rules.d/88-nuand-bladerf2.rules to /lib/udev/rules.d/88-nuand-bladerf2.rules.usr-is-merged by usr-is-merged', none removed.
No diversion 'diversion of /lib/udev/rules.d/88-nuand-bootloader.rules to /lib/udev/rules.d/88-nuand-bootloader.rules.usr-is-merged by usr-is-merged', none removed.
Setting up libsoapysdr0.8:amd64 (0.8.1-4build1) ...
Setting up libboost-serialization1.83.0:amd64 (1.83.0-2.1ubuntu3.2) ...
Setting up libsoapysdr-dev:amd64 (0.8.1-4build1) ...
Setting up libosmosdr0:amd64 (0.1.8.effcaa7-10build1) ...
No diversion 'diversion of /lib/udev/rules.d/60-libosmosdr0.rules to /lib/udev/rules.d/60-libosmosdr0.rules.usr-is-merged by usr-is-merged', none removed.
Setting up soapysdr0.8-module-airspy:amd64 (0.2.0-4) ...
Setting up libboost-chrono1.83.0t64:amd64 (1.83.0-2.1ubuntu3.2) ...
Setting up soapyosmo-common0.8:amd64 (0.2.5-8build3) ...
Setting up libnova-0.16-0t64:amd64 (0.16-5.1build1) ...
Setting up libopus0:amd64 (1.4-1build1) ...
Setting up soapysdr0.8-module-bladerf:amd64 (0.4.1-5) ...
Setting up liblimesuite23.11-1:amd64 (23.11.0+dfsg-2build2) ...
Setting up libindi-data (1.9.9+dfsg-3build3) ...
Setting up librtlsdr2:amd64 (2.0.1-2build1) ...
No diversion 'diversion of /lib/udev/rules.d/60-librtlsdr2.rules to /lib/udev/rules.d/60-librtlsdr2.rules.usr-is-merged by usr-is-merged', none removed.
Setting up soapysdr0.8-module-lms7:amd64 (23.11.0+dfsg-2build2) ...
Setting up libasyncns0:amd64 (0.8-6build4) ...
Setting up soapysdr0.8-module-mirisdr:amd64 (0.2.5-8build3) ...
Setting up libindiclient1:amd64 (1.9.9+dfsg-3build3) ...
Setting up libflac12t64:amd64 (1.4.3+ds-2.1ubuntu2) ...
Setting up libsamplerate0:amd64 (0.2.2-4build1) ...
Setting up libtecla1t64:amd64 (1.6.3-3.1build1) ...
Setting up libmp3lame0:amd64 (3.100-6build1) ...
Setting up libvorbisenc2:amd64 (1.3.7-1build3) ...
Setting up soapysdr0.8-module-hackrf:amd64 (0.3.4-1) ...
Setting up soapysdr0.8-module-redpitaya:amd64 (0.1.1-5) ...
Setting up soapysdr0.8-module-rfspace:amd64 (0.2.5-8build3) ...
Setting up soapysdr0.8-module-rtlsdr:amd64 (0.3.3-1build1) ...
Setting up soapysdr0.8-module-remote:amd64 (0.5.2-4) ...
Setting up soapysdr0.8-module-osmosdr:amd64 (0.2.5-8build3) ...
Setting up libuhd4.6.0t64:amd64 (4.6.0.0+ds1-5.1ubuntu0.24.04.1) ...
Setting up libhamlib4t64:amd64 (4.5.5-3.2build2) ...
Setting up soapysdr0.8-module-uhd:amd64 (0.4.1-4build4) ...
Setting up libjack-jackd2-0:amd64 (1.9.21~dfsg-3ubuntu3) ...
Setting up libsndfile1:amd64 (1.2.2-1ubuntu5.24.04.1) ...
Setting up bladerf (0.2023.02-4build1) ...
Setting up libpulse0:amd64 (1:16.1+dfsg1-2ubuntu10.1) ...
Setting up librtaudio6:amd64 (5.2.0~ds1-2build3) ...
Setting up soapysdr0.8-module-audio:amd64 (0.1.1-5build2) ...
Setting up soapysdr0.8-module-all:amd64 (0.8.1-4build1) ...
Processing triggers for man-db (2.12.0-4build2) ...
Not building database; man-db/auto-update is not 'true'.
Processing triggers for libc-bin (2.39-0ubuntu8.7) ...

Running kernel seems to be up-to-date.

No services need to be restarted.

No containers need to be restarted.

No user sessions are running outdated binaries.

No VM guests are running outdated hypervisor (qemu) binaries on this host.
>>> rustup toolchain install stable --profile minimal --component rustfmt
info: syncing channel updates for stable-x86_64-unknown-linux-gnu
info: latest update on 2026-07-16 for version 1.97.1 (8bab26f4f 2026-07-14)
info: removing previous version of component clippy
info: removing previous version of component rustfmt
info: removing previous version of component cargo
info: removing previous version of component rust-std
info: removing previous version of component rustc
info: downloading 5 components

  stable-x86_64-unknown-linux-gnu updated - rustc 1.97.1 (8bab26f4f 2026-07-14) (from rustc 1.97.0 (2d8144b78 2026-07-07))

>>> rustup default stable
info: using existing install for stable-x86_64-unknown-linux-gnu
info: default toolchain set to stable-x86_64-unknown-linux-gnu

  stable-x86_64-unknown-linux-gnu unchanged - rustc 1.97.1 (8bab26f4f 2026-07-14)

>>> rustfmt --edition 2024 crates/tetra-config/src/bluestation/config.rs crates/tetra-config/src/bluestation/sec_cell.rs crates/tetra-entities/src/sndcp/ip.rs crates/tetra-entities/src/sndcp/mod.rs crates/tetra-entities/src/sndcp/sndcp_bs.rs crates/tetra-entities/src/sndcp/wap.rs crates/tetra-entities/src/umac/subcomp/bs_sched.rs crates/tetra-entities/src/umac/umac_bs.rs crates/tetra-entities/tests/common/default_stack.rs
>>> cargo check --locked -p tetra-config
    Updating crates.io index
 Downloading crates ...
  Downloaded arrayvec v0.7.6
  Downloaded autocfg v1.5.0
  Downloaded bitcode_derive v0.6.9
  Downloaded aho-corasick v1.1.4
  Downloaded bitcode v0.6.9
  Downloaded chrono v0.4.44
  Downloaded memchr v2.8.0
  Downloaded bytemuck v1.25.0
  Downloaded itoa v1.0.18
  Downloaded matchers v0.2.0
  Downloaded nu-ansi-term v0.50.3
  Downloaded crossbeam-channel v0.5.15
  Downloaded num-conv v0.2.1
  Downloaded once_cell v1.21.4
  Downloaded crossbeam-utils v0.8.21
  Downloaded deranged v0.5.8
  Downloaded log v0.4.29
  Downloaded phf v0.12.1
  Downloaded pin-project-lite v0.2.17
  Downloaded num-traits v0.2.19
  Downloaded siphasher v1.0.2
  Downloaded indexmap v2.13.0
  Downloaded smallvec v1.15.1
  Downloaded toml_datetime v0.6.11
  Downloaded toml_write v0.1.2
  Downloaded tracing-attributes v0.1.31
  Downloaded tracing-core v0.1.36
  Downloaded tracing-log v0.2.0
  Downloaded const_format_proc_macros v0.2.34
  Downloaded equivalent v1.0.2
  Downloaded lazy_static v1.5.0
  Downloaded phf_shared v0.12.1
  Downloaded serde v1.0.228
  Downloaded thiserror-impl v2.0.18
  Downloaded iana-time-zone v0.1.65
  Downloaded tracing-appender v0.2.4
  Downloaded git-version v0.3.9
  Downloaded git-version-macro v0.3.9
  Downloaded serde_spanned v0.6.9
  Downloaded winnow v0.7.15
  Downloaded powerfmt v0.2.0
  Downloaded unicode-ident v1.0.24
  Downloaded const_format v0.2.35
  Downloaded hashbrown v0.16.1
  Downloaded sharded-slab v0.1.7
  Downloaded toml v0.8.23
  Downloaded cfg-if v1.0.4
  Downloaded serde_core v1.0.228
  Downloaded toml_edit v0.22.27
  Downloaded unicode-xid v0.2.6
  Downloaded thiserror v2.0.18
  Downloaded time-core v0.1.8
  Downloaded time-macros v0.2.27
  Downloaded proc-macro2 v1.0.106
  Downloaded quote v1.0.45
  Downloaded time v0.3.47
  Downloaded syn v2.0.117
  Downloaded serde_derive v1.0.228
  Downloaded regex-syntax v0.8.10
  Downloaded chrono-tz v0.10.4
  Downloaded thread_local v1.1.9
  Downloaded tracing-subscriber v0.3.23
  Downloaded tracing v0.1.44
  Downloaded regex-automata v0.4.14
  Downloaded glam v0.32.1
   Compiling proc-macro2 v1.0.106
   Compiling quote v1.0.45
   Compiling unicode-ident v1.0.24
   Compiling serde_core v1.0.228
   Compiling serde v1.0.228
    Checking once_cell v1.21.4
   Compiling autocfg v1.5.0
    Checking tracing-core v0.1.36
    Checking regex-syntax v0.8.10
   Compiling num-traits v0.2.19
   Compiling syn v2.0.117
    Checking powerfmt v0.2.0
   Compiling crossbeam-utils v0.8.21
    Checking deranged v0.5.8
    Checking regex-automata v0.4.14
    Checking cfg-if v1.0.4
    Checking pin-project-lite v0.2.17
   Compiling thiserror v2.0.18
    Checking log v0.4.29
    Checking lazy_static v1.5.0
    Checking itoa v1.0.18
    Checking num-conv v0.2.1
    Checking siphasher v1.0.2
    Checking time-core v0.1.8
    Checking phf_shared v0.12.1
    Checking time v0.3.47
    Checking sharded-slab v0.1.7
    Checking matchers v0.2.0
    Checking tracing-log v0.2.0
    Checking thread_local v1.1.9
    Checking equivalent v1.0.2
    Checking hashbrown v0.16.1
    Checking smallvec v1.15.1
   Compiling unicode-xid v0.2.6
    Checking iana-time-zone v0.1.65
    Checking nu-ansi-term v0.50.3
   Compiling chrono-tz v0.10.4
    Checking chrono v0.4.44
    Checking indexmap v2.13.0
   Compiling const_format_proc_macros v0.2.34
    Checking crossbeam-channel v0.5.15
    Checking phf v0.12.1
    Checking toml_write v0.1.2
    Checking bytemuck v1.25.0
    Checking winnow v0.7.15
    Checking const_format v0.2.35
   Compiling serde_derive v1.0.228
   Compiling tracing-attributes v0.1.31
   Compiling thiserror-impl v2.0.18
    Checking tracing v0.1.44
    Checking tracing-subscriber v0.3.23
   Compiling git-version-macro v0.3.9
   Compiling bitcode_derive v0.6.9
    Checking git-version v0.3.9
    Checking tracing-appender v0.2.4
    Checking bitcode v0.6.9
    Checking serde_spanned v0.6.9
    Checking toml_datetime v0.6.11
    Checking toml_edit v0.22.27
    Checking tetra-core v1.3.0 (/home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-core)
    Checking toml v0.8.23
    Checking tetra-config v1.3.0 (/home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-config)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.13s
>>> cargo check --locked -p tetra-entities
 Downloading crates ...
  Downloaded futures-sink v0.3.32
  Downloaded generic-array v0.14.7
  Downloaded cpufeatures v0.2.17
  Downloaded tinyvec_macros v0.1.1
  Downloaded as-any v0.3.2
  Downloaded atomic-waker v1.1.2
  Downloaded cfg_aliases v0.2.1
  Downloaded futures-io v0.3.32
  Downloaded futures-task v0.3.32
  Downloaded tower-layer v0.3.3
  Downloaded byteorder v1.5.0
  Downloaded block-buffer v0.10.4
  Downloaded http-body v1.0.1
  Downloaded synstructure v0.13.2
  Downloaded http-body-util v0.1.3
  Downloaded bitflags v2.11.0
  Downloaded tower-service v0.3.3
  Downloaded tower v0.5.3
  Downloaded tokio-rustls v0.26.4
  Downloaded form_urlencoded v1.2.2
  Downloaded untrusted v0.9.0
  Downloaded futures-core v0.3.32
  Downloaded transpose v0.2.3
  Downloaded utf8_iter v1.0.4
  Downloaded zerofrom-derive v0.1.7
  Downloaded futures-channel v0.3.32
  Downloaded httparse v1.10.1
  Downloaded potential_utf v0.1.5
  Downloaded hyper-rustls v0.27.9
  Downloaded find-msvc-tools v0.1.9
  Downloaded stable_deref_trait v1.2.1
  Downloaded tinyvec v1.11.0
  Downloaded zerofrom v0.1.8
  Downloaded md5 v0.7.0
  Downloaded rand_chacha v0.3.1
  Downloaded rustc-hash v2.1.2
  Downloaded utf-8 v0.7.6
  Downloaded base64 v0.22.1
  Downloaded bytes v1.11.1
  Downloaded num v0.4.3
  Downloaded serde_urlencoded v0.7.1
  Downloaded try-lock v0.2.5
  Downloaded writeable v0.6.3
  Downloaded crypto-common v0.1.7
  Downloaded want v0.3.1
  Downloaded cc v1.2.58
  Downloaded getrandom v0.2.17
  Downloaded yoke-derive v0.8.2
  Downloaded lru-slab v0.1.2
  Downloaded openssl-probe v0.1.6
  Downloaded openssl-probe v0.2.1
  Downloaded percent-encoding v2.3.2
  Downloaded sync_wrapper v1.0.2
  Downloaded sha1 v0.10.6
  Downloaded subtle v2.6.1
  Downloaded zeroize v1.8.2
  Downloaded zerovec-derive v0.11.3
  Downloaded rustls-native-certs v0.8.3
  Downloaded data-encoding v2.10.0
  Downloaded num-iter v0.1.45
  Downloaded primal-check v0.3.4
  Downloaded displaydoc v0.2.6
  Downloaded idna_adapter v1.2.2
  Downloaded num-integer v0.1.46
  Downloaded rustls-pki-types v1.14.0
  Downloaded tinystr v0.8.3
  Downloaded fastbloom v0.14.1
  Downloaded getrandom v0.4.2
  Downloaded num-rational v0.4.2
  Downloaded ppv-lite86 v0.2.21
  Downloaded rand_chacha v0.9.0
  Downloaded digest v0.10.7
  Downloaded quinn-udp v0.5.14
  Downloaded rand_core v0.9.5
  Downloaded rustls-pemfile v2.2.0
  Downloaded soapysdr-sys v0.8.1
  Downloaded strength_reduce v0.2.4
  Downloaded thiserror v1.0.69
  Downloaded thiserror-impl v1.0.69
  Downloaded num-complex v0.4.6
  Downloaded rand_core v0.6.4
  Downloaded rustls-native-certs v0.7.3
  Downloaded soapysdr v0.5.0
  Downloaded version_check v0.9.5
  Downloaded ryu v1.0.23
  Downloaded zmij v1.0.21
  Downloaded litemap v0.8.2
  Downloaded ipnet v2.12.0
  Downloaded pkg-config v0.3.32
  Downloaded shlex v1.3.0
  Downloaded slab v0.4.12
  Downloaded yoke v0.8.2
  Downloaded getrandom v0.3.4
  Downloaded http v1.4.0
  Downloaded icu_collections v2.2.0
  Downloaded socket2 v0.6.3
  Downloaded icu_properties v2.2.0
  Downloaded icu_provider v2.2.0
  Downloaded rustls-platform-verifier v0.6.2
  Downloaded icu_normalizer_data v2.2.0
  Downloaded rand v0.8.5
  Downloaded uuid v1.23.0
  Downloaded hyper-util v0.1.20
  Downloaded icu_locale_core v2.2.0
  Downloaded rustls-webpki v0.103.10
  Downloaded mio v1.2.0
  Downloaded quinn v0.11.9
  Downloaded tungstenite v0.24.0
  Downloaded typenum v1.19.0
  Downloaded hyper v1.10.1
  Downloaded icu_normalizer v2.2.0
  Downloaded num-bigint v0.4.6
  Downloaded rand v0.9.2
  Downloaded zerotrie v0.2.4
  Downloaded futures-util v0.3.32
  Downloaded reqwest v0.12.28
  Downloaded tower-http v0.6.11
  Downloaded url v2.5.8
  Downloaded idna v1.1.0
  Downloaded libm v0.2.16
  Downloaded serde_json v1.0.149
  Downloaded webpki-roots v1.0.7
  Downloaded icu_properties_data v2.2.0
  Downloaded zerovec v0.11.6
  Downloaded quinn-proto v0.11.14
  Downloaded rustfft v6.4.1
  Downloaded rustls v0.23.37
  Downloaded zerocopy v0.8.48
  Downloaded libc v0.2.183
  Downloaded tokio v1.50.0
  Downloaded ring v0.17.14
   Compiling libc v0.2.183
   Compiling syn v2.0.117
   Compiling num-traits v0.2.19
    Checking smallvec v1.15.1
    Checking stable_deref_trait v1.2.1
    Checking bytes v1.11.1
   Compiling shlex v1.3.0
   Compiling find-msvc-tools v0.1.9
    Checking zeroize v1.8.2
    Checking rustls-pki-types v1.14.0
   Compiling cc v1.2.58
   Compiling zerocopy v0.8.48
    Checking getrandom v0.2.17
    Checking socket2 v0.6.3
    Checking futures-core v0.3.32
    Checking untrusted v0.9.0
    Checking mio v1.2.0
    Checking writeable v0.6.3
    Checking litemap v0.8.2
   Compiling rustls v0.23.37
    Checking tokio v1.50.0
   Compiling ring v0.17.14
   Compiling synstructure v0.13.2
    Checking ppv-lite86 v0.2.21
   Compiling zerovec-derive v0.11.3
   Compiling tracing-attributes v0.1.31
   Compiling zerofrom-derive v0.1.7
   Compiling yoke-derive v0.8.2
   Compiling displaydoc v0.2.6
    Checking tracing v0.1.44
   Compiling serde_derive v1.0.228
    Checking zerofrom v0.1.8
    Checking yoke v0.8.2
   Compiling thiserror-impl v2.0.18
    Checking zerovec v0.11.6
    Checking tinystr v0.8.3
    Checking icu_locale_core v2.2.0
    Checking rustls-webpki v0.103.10
    Checking potential_utf v0.1.5
    Checking zerotrie v0.2.4
    Checking http v1.4.0
    Checking slab v0.4.12
   Compiling icu_properties_data v2.2.0
   Compiling icu_normalizer_data v2.2.0
   Compiling typenum v1.19.0
   Compiling getrandom v0.3.4
    Checking subtle v2.6.1
    Checking utf8_iter v1.0.4
   Compiling version_check v0.9.5
    Checking futures-sink v0.3.32
    Checking icu_collections v2.2.0
   Compiling generic-array v0.14.7
    Checking icu_provider v2.2.0
    Checking thiserror v2.0.18
    Checking num-integer v0.1.46
    Checking percent-encoding v2.3.2
    Checking memchr v2.8.0
   Compiling httparse v1.10.1
    Checking http-body v1.0.1
    Checking serde v1.0.228
    Checking futures-task v0.3.32
   Compiling cfg_aliases v0.2.1
    Checking futures-io v0.3.32
    Checking chrono v0.4.44
    Checking futures-util v0.3.32
    Checking tracing-subscriber v0.3.23
    Checking rand_core v0.9.5
    Checking icu_normalizer v2.2.0
    Checking icu_properties v2.2.0
   Compiling git-version-macro v0.3.9
   Compiling bitcode_derive v0.6.9
    Checking try-lock v0.2.5
    Checking tower-service v0.3.3
   Compiling getrandom v0.4.2
   Compiling libm v0.2.16
    Checking idna_adapter v1.2.2
    Checking want v0.3.1
    Checking tracing-appender v0.2.4
    Checking git-version v0.3.9
    Checking rand_chacha v0.9.0
    Checking chrono-tz v0.10.4
    Checking serde_spanned v0.6.9
    Checking toml_datetime v0.6.11
    Checking form_urlencoded v1.2.2
    Checking futures-channel v0.3.32
   Compiling pkg-config v0.3.32
    Checking openssl-probe v0.2.1
   Compiling zmij v1.0.21
    Checking atomic-waker v1.1.2
    Checking toml_edit v0.22.27
   Compiling soapysdr-sys v0.8.1
    Checking bitcode v0.6.9
    Checking hyper v1.10.1
    Checking rustls-native-certs v0.8.3
    Checking rand v0.9.2
    Checking idna v1.1.0
    Checking block-buffer v0.10.4
    Checking crypto-common v0.1.7
   Compiling quinn-udp v0.5.14
    Checking sync_wrapper v1.0.2
    Checking rand_core v0.6.4
    Checking num-complex v0.4.6
   Compiling serde_json v1.0.149
   Compiling thiserror v1.0.69
    Checking tower-layer v0.3.3
    Checking tinyvec_macros v0.1.1
    Checking ipnet v2.12.0
    Checking base64 v0.22.1
    Checking tower v0.5.3
    Checking fastbloom v0.14.1
    Checking hyper-util v0.1.20
    Checking tinyvec v1.11.0
    Checking rand_chacha v0.3.1
    Checking digest v0.10.7
    Checking toml v0.8.23
    Checking uuid v1.23.0
    Checking url v2.5.8
    Checking rustls-platform-verifier v0.6.2
    Checking tokio-rustls v0.26.4
   Compiling quinn v0.11.9
    Checking num-bigint v0.4.6
   Compiling thiserror-impl v1.0.69
    Checking webpki-roots v1.0.7
    Checking rustls-pemfile v2.2.0
    Checking strength_reduce v0.2.4
    Checking rustc-hash v2.1.2
    Checking ryu v1.0.23
    Checking bitflags v2.11.0
    Checking cpufeatures v0.2.17
    Checking lru-slab v0.1.2
    Checking openssl-probe v0.1.6
    Checking rustls-native-certs v0.7.3
    Checking quinn-proto v0.11.14
    Checking num-rational v0.4.2
    Checking sha1 v0.10.6
    Checking tower-http v0.6.11
    Checking tetra-core v1.3.0 (/home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-core)
    Checking serde_urlencoded v0.7.1
    Checking transpose v0.2.3
    Checking hyper-rustls v0.27.9
    Checking rand v0.8.5
    Checking tetra-config v1.3.0 (/home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-config)
    Checking tetra-saps v1.3.0 (/home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-saps)
    Checking http-body-util v0.1.3
    Checking primal-check v0.3.4
    Checking num-iter v0.1.45
    Checking data-encoding v2.10.0
   Compiling tetra-entities v1.3.0 (/home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities)
    Checking utf-8 v0.7.6
    Checking byteorder v1.5.0
    Checking num v0.4.3
    Checking reqwest v0.12.28
    Checking tungstenite v0.24.0
    Checking rustfft v6.4.1
    Checking soapysdr v0.5.0
    Checking as-any v0.3.2
    Checking md5 v0.7.0
    Checking tetra-pdus v1.3.0 (/home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-pdus)
error[E0432]: unresolved import `tetra_pdus::llc::pdus::al_data`
 --> crates/tetra-entities/src/umac/umac_bs.rs:8:29
  |
8 | use tetra_pdus::llc::pdus::{al_data::AlData, bl_adata::BlAdata, bl_data::BlData, bl_udata::BlUdata};
  |                             ^^^^^^^ could not find `al_data` in `pdus`

For more information about this error, try `rustc --explain E0432`.
error: could not compile `tetra-entities` (lib) due to 1 previous error
```
