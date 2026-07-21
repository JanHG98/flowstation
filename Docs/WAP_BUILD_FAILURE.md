# WAP-Port Buildfehler

Workflow-Exitcode: 1

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
Get:8 https://dl.google.com/linux/chrome-stable/deb stable InRelease [1825 B]
Get:4 http://azure.archive.ubuntu.com/ubuntu noble-backports InRelease [126 kB]
Get:5 http://azure.archive.ubuntu.com/ubuntu noble-security InRelease [126 kB]
Get:9 https://packages.microsoft.com/ubuntu/24.04/prod noble/main arm64 Packages [200 kB]
Get:10 https://packages.microsoft.com/ubuntu/24.04/prod noble/main amd64 Packages [233 kB]
Get:11 https://packages.microsoft.com/ubuntu/24.04/prod noble/main armhf Packages [11.7 kB]
Get:12 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 Packages [1122 kB]
Get:21 https://dl.google.com/linux/chrome-stable/deb stable/main amd64 Packages [1414 B]
Get:13 http://azure.archive.ubuntu.com/ubuntu noble-updates/main Translation-en [274 kB]
Get:14 http://azure.archive.ubuntu.com/ubuntu noble-updates/main amd64 Components [180 kB]
Get:15 http://azure.archive.ubuntu.com/ubuntu noble-updates/universe amd64 Packages [1666 kB]
Get:16 http://azure.archive.ubuntu.com/ubuntu noble-updates/universe Translation-en [329 kB]
Get:17 http://azure.archive.ubuntu.com/ubuntu noble-updates/universe amd64 Components [388 kB]
Get:18 http://azure.archive.ubuntu.com/ubuntu noble-updates/restricted amd64 Packages [1274 kB]
Get:19 http://azure.archive.ubuntu.com/ubuntu noble-updates/restricted Translation-en [291 kB]
Get:20 http://azure.archive.ubuntu.com/ubuntu noble-updates/multiverse amd64 Components [940 B]
Get:22 http://azure.archive.ubuntu.com/ubuntu noble-backports/main amd64 Components [5752 B]
Get:23 http://azure.archive.ubuntu.com/ubuntu noble-backports/universe amd64 Components [10.5 kB]
Get:24 http://azure.archive.ubuntu.com/ubuntu noble-security/main amd64 Packages [861 kB]
Get:25 http://azure.archive.ubuntu.com/ubuntu noble-security/main Translation-en [192 kB]
Get:26 http://azure.archive.ubuntu.com/ubuntu noble-security/main amd64 Components [46.3 kB]
Get:27 http://azure.archive.ubuntu.com/ubuntu noble-security/universe amd64 Packages [1180 kB]
Get:28 http://azure.archive.ubuntu.com/ubuntu noble-security/universe Translation-en [233 kB]
Get:29 http://azure.archive.ubuntu.com/ubuntu noble-security/universe amd64 Components [76.3 kB]
Get:30 http://azure.archive.ubuntu.com/ubuntu noble-security/restricted amd64 Packages [1178 kB]
Get:31 http://azure.archive.ubuntu.com/ubuntu noble-security/restricted Translation-en [272 kB]
Fetched 10.4 MB in 1s (7589 kB/s)
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
Fetched 11.2 MB in 1s (19.2 MB/s)
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

>>> rustfmt --edition 2024 --check crates/tetra-config/src/bluestation/config.rs crates/tetra-config/src/bluestation/sec_cell.rs crates/tetra-entities/src/sndcp/ip.rs crates/tetra-entities/src/sndcp/mod.rs crates/tetra-entities/src/sndcp/sndcp_bs.rs crates/tetra-entities/src/sndcp/wap.rs crates/tetra-entities/src/umac/subcomp/bs_sched.rs crates/tetra-entities/src/umac/umac_bs.rs crates/tetra-entities/tests/common/default_stack.rs
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-config/src/bluestation/config.rs:3:
 use tetra_core::freqs::FreqInfo;
 
 use crate::bluestation::{
-    CfgAsterisk, CfgCellInfo, CfgControl, CfgControlRoom, CfgDapnet, CfgEcholink, CfgEmergency, CfgGeoalarm, CfgHealth, CfgMeshcom, CfgNetInfo, CfgPhyIo,
-    CfgAudioPlayer, CfgRecording, CfgRecovery, CfgTts, CfgSecurity, CfgSnomNotify, CfgTpg2200Action, CfgWxService, PhyBackend, StackState,
+    CfgAsterisk, CfgAudioPlayer, CfgCellInfo, CfgControl, CfgControlRoom, CfgDapnet, CfgEcholink, CfgEmergency, CfgGeoalarm, CfgHealth,
+    CfgMeshcom, CfgNetInfo, CfgPhyIo, CfgRecording, CfgRecovery, CfgSecurity, CfgSnomNotify, CfgTpg2200Action, CfgTts, CfgWxService,
+    PhyBackend, StackState,
 };
 
 use super::sec_brew::CfgBrew;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-config/src/bluestation/config.rs:247:
         if self.cell.wap_ip.enabled && self.cell.wap_ip.response_ttl == 0 {
             return Err("cell_info.wap_ip.response_ttl must be 1..255");
         }
-        if self.cell.wap_ip.enabled
-            && !(1..=1024).contains(&self.cell.wap_ip.max_request_payload_bytes)
-        {
+        if self.cell.wap_ip.enabled && !(1..=1024).contains(&self.cell.wap_ip.max_request_payload_bytes) {
             return Err("cell_info.wap_ip.max_request_payload_bytes must be 1..1024");
         }
         if self.cell.wap_ip.enabled
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-config/src/bluestation/sec_cell.rs:39:
     pub text: Option<String>,
 }
 
-
 /// Built-in SNDCP WAP/IP endpoint configuration.
 #[derive(Debug, Clone)]
 pub struct CfgWapIp {
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-config/src/bluestation/sec_cell.rs:94:
 }
 
 fn parse_ipv4_prefix(value: Option<String>, default: [u8; 3]) -> [u8; 3] {
-    let Some(value) = value else { return default; };
+    let Some(value) = value else {
+        return default;
+    };
     let mut parts = value.split('.').filter_map(|part| part.parse::<u8>().ok());
     match (parts.next(), parts.next(), parts.next(), parts.next()) {
         (Some(a), Some(b), Some(c), None) => [a, b, c],
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-config/src/bluestation/sec_cell.rs:103:
 }
 
 fn wap_ip_dto_to_cfg(dto: Option<WapIpDto>) -> CfgWapIp {
-    let Some(dto) = dto else { return CfgWapIp::default(); };
+    let Some(dto) = dto else {
+        return CfgWapIp::default();
+    };
     let defaults = CfgWapIp::default();
     CfgWapIp {
         enabled: dto.enabled.unwrap_or(false),
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/ip.rs:147:
 
     #[test]
     fn builds_reference_udp_vector() {
-        let packet = build_ipv4_udp_npdu(
-            [192, 0, 2, 1],
-            [192, 0, 2, 2],
-            49_152,
-            9_200,
-            b"wap",
-            7,
-            32,
-        )
-        .unwrap();
-        assert_eq!(
-            packet,
-            hex("4500001f00070000201116c4c0000201c0000202c00023f0000b0000776170")
-        );
+        let packet = build_ipv4_udp_npdu([192, 0, 2, 1], [192, 0, 2, 2], 49_152, 9_200, b"wap", 7, 32).unwrap();
+        assert_eq!(packet, hex("4500001f00070000201116c4c0000201c0000202c00023f0000b0000776170"));
         assert_eq!(internet_checksum(&packet[..20]), 0);
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/ip.rs:147:
 
     #[test]
     fn builds_reference_udp_vector() {
-        let packet = build_ipv4_udp_npdu(
-            [192, 0, 2, 1],
-            [192, 0, 2, 2],
-            49_152,
-            9_200,
-            b"wap",
-            7,
-            32,
-        )
-        .unwrap();
-        assert_eq!(
-            packet,
-            hex("4500001f00070000201116c4c0000201c0000202c00023f0000b0000776170")
-        );
+        let packet = build_ipv4_udp_npdu([192, 0, 2, 1], [192, 0, 2, 2], 49_152, 9_200, b"wap", 7, 32).unwrap();
+        assert_eq!(packet, hex("4500001f00070000201116c4c0000201c0000202c00023f0000b0000776170"));
         assert_eq!(internet_checksum(&packet[..20]), 0);
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:79:
 
     fn handle_indication(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd) {
         if !self.config.config().cell.sndcp_service {
-            tracing::debug!("SNDCP: service disabled; ignoring packet-data PDU from {}", ind.received_tetra_address);
+            tracing::debug!(
+                "SNDCP: service disabled; ignoring packet-data PDU from {}",
+                ind.received_tetra_address
+            );
             return;
         }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:110:
             SN_RECONNECT => self.handle_reconnect(queue, ind, &pdu),
             SN_UNITDATA | SN_DATA => self.handle_user_data(queue, ind, &mut pdu),
             _ => {
-                tracing::warn!("SNDCP/WAP: unsupported inbound SN-PDU type {} from {}", sn_type, ind.received_tetra_address);
+                tracing::warn!(
+                    "SNDCP/WAP: unsupported inbound SN-PDU type {} from {}",
+                    sn_type,
+                    ind.received_tetra_address
+                );
                 Ok(())
             }
         };
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:119:
         }
     }
 
-    fn handle_activate(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_activate(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let nsapi_hint = pdu.peek_bits_startoffset(8, 4).map(|value| value as u8);
         let demand = match decode_activate_demand(pdu) {
             Ok(demand) => demand,
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:138:
 
         let issi = ind.received_tetra_address.ssi;
         let key = (issi, demand.nsapi);
-        if !self.contexts.contains_key(&key)
-            && self.contexts.keys().filter(|(context_issi, _)| *context_issi == issi).count() >= 4
-        {
+        if !self.contexts.contains_key(&key) && self.contexts.keys().filter(|(context_issi, _)| *context_issi == issi).count() >= 4 {
             self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 19), None);
             return Err("maximum of four PDP contexts per ISSI exceeded".into());
         }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:156:
                     return Err(format!("invalid static IPv4 address {address:?}"));
                 }
                 let in_use = self.contexts.iter().any(|(&(context_issi, context_nsapi), context)| {
-                    (context_issi, context_nsapi) != (ind.received_tetra_address.ssi, demand.nsapi)
-                        && context.address == address
+                    (context_issi, context_nsapi) != (ind.received_tetra_address.ssi, demand.nsapi) && context.address == address
                 });
                 if in_use {
                     self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 9), None);
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:197:
         Ok(())
     }
 
-    fn handle_deactivate(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_deactivate(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let mut reader = reader(pdu);
         expect(&mut reader, 4, SN_DEACTIVATE_DEMAND as u64, "deactivate type")?;
         let selector = read(&mut reader, 8, "deactivation selector")? as u8;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:232:
         Ok(())
     }
 
-    fn handle_data_transmit(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_data_transmit(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let mut reader = reader(pdu);
         expect(&mut reader, 4, SN_DATA_TRANSMIT_REQUEST as u64, "data transmit type")?;
         let nsapi = read(&mut reader, 4, "NSAPI")? as u8;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:250:
         context.state = PacketState::Ready;
         let response = encode_data_transmit_response(nsapi, true, None);
         self.send_control(queue, ind, response, Some(packet_data_channel()));
-        tracing::info!("SNDCP/WAP: ISSI={} NSAPI={} entered READY on TS2", ind.received_tetra_address.ssi, nsapi);
+        tracing::info!(
+            "SNDCP/WAP: ISSI={} NSAPI={} entered READY on TS2",
+            ind.received_tetra_address.ssi,
+            nsapi
+        );
         Ok(())
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:257:
-    fn handle_reconnect(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_reconnect(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let mut reader = reader(pdu);
         expect(&mut reader, 4, SN_RECONNECT as u64, "reconnect type")?;
         let has_data = read(&mut reader, 1, "data to send")? != 0;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:282:
         }
     }
 
-    fn handle_end_of_data(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_end_of_data(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let mut reader = reader(pdu);
         expect(&mut reader, 4, SN_END_OF_DATA as u64, "end-of-data type")?;
         let immediate = read(&mut reader, 1, "immediate service change")?;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:307:
         Ok(())
     }
 
-    fn handle_user_data(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &mut BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_user_data(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &mut BitBuffer) -> Result<(), String> {
         if !self.wap.enabled {
             return Err("WAP/IP endpoint is disabled".into());
         }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:332:
             .ok_or_else(|| "truncated N-PDU".to_string())?;
 
         let key = (ind.received_tetra_address.ssi, nsapi);
-        let context = self.contexts.get(&key).copied().ok_or_else(|| format!("missing PDP context for NSAPI {nsapi}"))?;
+        let context = self
+            .contexts
+            .get(&key)
+            .copied()
+            .ok_or_else(|| format!("missing PDP context for NSAPI {nsapi}"))?;
         let request_source = npdu.get(12..16).ok_or_else(|| "IPv4 N-PDU too short".to_string())?;
         if request_source != &context.address[..] {
             return Err(format!(
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:411:
         }
     }
 
-    fn send_control(
-        &self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        sn_pdu: BitBuffer,
-        chan_alloc: Option<CmceChanAllocReq>,
-    ) {
+    fn send_control(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, sn_pdu: BitBuffer, chan_alloc: Option<CmceChanAllocReq>) {
         queue.push_back(SapMsg {
             sap: Sap::TlaSap,
             src: TetraEntity::Sndcp,
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:87:
 }
 
 /// Build a complete IPv4/UDP response. `Ok(None)` means the inbound WTP PDU was an ACK/ABORT.
-pub fn build_response(
-    request_npdu: &[u8],
-    endpoint: WapEndpoint,
-    snapshot: &WapStatusSnapshot,
-) -> Result<Option<Vec<u8>>, WapError> {
+pub fn build_response(request_npdu: &[u8], endpoint: WapEndpoint, snapshot: &WapStatusSnapshot) -> Result<Option<Vec<u8>>, WapError> {
     let ip = parse_ipv4_packet(request_npdu)?;
     if ip.fragmented {
         return Err(WapError::Fragmented);
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:212:
                 let len = *payload.get(offset + 1).ok_or(WapError::MalformedWtpWsp)? as usize;
                 offset = offset.checked_add(2 + len).ok_or(WapError::MalformedWtpWsp)?;
             } else {
-                offset = offset
-                    .checked_add(1 + (h & 0x03) as usize)
-                    .ok_or(WapError::MalformedWtpWsp)?;
+                offset = offset.checked_add(1 + (h & 0x03) as usize).ok_or(WapError::MalformedWtpWsp)?;
             }
             if offset > payload.len() {
                 return Err(WapError::MalformedWtpWsp);
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:338:
     let state = escape(&snapshot.service_state, 12);
     let uptime = compact_uptime(snapshot.uptime_secs);
     let body = match sector {
-        0 => format!("{title}<br/>{state} MS:{} C:{}<br/>Up:{uptime}<br/><a href=\"?s=1\">N</a>", snapshot.registered_ms, snapshot.active_calls),
-        1 => format!("Net {}/{}<br/>Carrier:{}<br/>SDS:{}<br/><a href=\"?s=2\">N</a> <a href=\"?s=0\">H</a>", snapshot.mcc, snapshot.mnc, snapshot.carrier, snapshot.queued_sds),
+        0 => format!(
+            "{title}<br/>{state} MS:{} C:{}<br/>Up:{uptime}<br/><a href=\"?s=1\">N</a>",
+            snapshot.registered_ms, snapshot.active_calls
+        ),
+        1 => format!(
+            "Net {}/{}<br/>Carrier:{}<br/>SDS:{}<br/><a href=\"?s=2\">N</a> <a href=\"?s=0\">H</a>",
+            snapshot.mcc, snapshot.mnc, snapshot.carrier, snapshot.queued_sds
+        ),
         _ => format!("Packet data OK<br/>WTP/WSP active<br/>UDP 9200<br/><a href=\"?s=0\">H</a>"),
     };
     let candidates = match format {
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:346:
         PageFormat::Xhtml => vec![
             format!("<html><body>{body}</body></html>"),
-            format!("<html><body>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></body></html>", snapshot.registered_ms),
+            format!(
+                "<html><body>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></body></html>",
+                snapshot.registered_ms
+            ),
             format!("<html><body>{title}<br/>{state}</body></html>"),
         ],
         PageFormat::Wml => vec![
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:352:
             format!("<wml><card><p>{body}</p></card></wml>"),
-            format!("<wml><card><p>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></p></card></wml>", snapshot.registered_ms),
+            format!(
+                "<wml><card><p>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></p></card></wml>",
+                snapshot.registered_ms
+            ),
             format!("<wml><card><p>{title}<br/>{state}</p></card></wml>"),
         ],
     };
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:437:
     #[test]
     fn connect_reply_matches_openwave_shape() {
         let caps = vec![
-            Capability { id: 0x80, value: vec![0x94, 0x80, 0x00] },
-            Capability { id: 0x81, value: vec![0x94, 0x80, 0x00] },
+            Capability {
+                id: 0x80,
+                value: vec![0x94, 0x80, 0x00],
+            },
+            Capability {
+                id: 0x81,
+                value: vec![0x94, 0x80, 0x00],
+            },
         ];
         assert_eq!(
             build_wtp_result(0x13cc, &build_connect_reply(&caps)),
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:445:
-            vec![0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21]
+            vec![
+                0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21
+            ]
         );
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:454:
     #[test]
     fn complete_ipv4_udp_connect_roundtrip_is_byte_exact() {
         let request_payload = vec![
-            0x08, 0x13, 0xcc, 0x12,
-            0x01, 0x10, 0x08, 0x00,
-            0x03, 0x80, 0x84, 0x21,
-            0x03, 0x81, 0x84, 0x21,
+            0x08, 0x13, 0xcc, 0x12, 0x01, 0x10, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21,
         ];
-        let request = build_ipv4_udp_npdu(
-            [10, 0, 0, 2],
-            [10, 0, 0, 1],
-            49_152,
-            9_200,
-            &request_payload,
-            0x2222,
-            64,
-        )
-        .unwrap();
+        let request = build_ipv4_udp_npdu([10, 0, 0, 2], [10, 0, 0, 1], 49_152, 9_200, &request_payload, 0x2222, 64).unwrap();
         let snapshot = WapStatusSnapshot {
             title: "NetCore-TETRA".into(),
             service_state: "ON AIR".into(),
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:503:
         assert_eq!(udp.destination_port, 49_152);
         assert_eq!(
             udp.payload,
-            &[0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21]
+            &[
+                0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21
+            ]
         );
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:79:
 
     fn handle_indication(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd) {
         if !self.config.config().cell.sndcp_service {
-            tracing::debug!("SNDCP: service disabled; ignoring packet-data PDU from {}", ind.received_tetra_address);
+            tracing::debug!(
+                "SNDCP: service disabled; ignoring packet-data PDU from {}",
+                ind.received_tetra_address
+            );
             return;
         }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:110:
             SN_RECONNECT => self.handle_reconnect(queue, ind, &pdu),
             SN_UNITDATA | SN_DATA => self.handle_user_data(queue, ind, &mut pdu),
             _ => {
-                tracing::warn!("SNDCP/WAP: unsupported inbound SN-PDU type {} from {}", sn_type, ind.received_tetra_address);
+                tracing::warn!(
+                    "SNDCP/WAP: unsupported inbound SN-PDU type {} from {}",
+                    sn_type,
+                    ind.received_tetra_address
+                );
                 Ok(())
             }
         };
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:119:
         }
     }
 
-    fn handle_activate(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_activate(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let nsapi_hint = pdu.peek_bits_startoffset(8, 4).map(|value| value as u8);
         let demand = match decode_activate_demand(pdu) {
             Ok(demand) => demand,
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:138:
 
         let issi = ind.received_tetra_address.ssi;
         let key = (issi, demand.nsapi);
-        if !self.contexts.contains_key(&key)
-            && self.contexts.keys().filter(|(context_issi, _)| *context_issi == issi).count() >= 4
-        {
+        if !self.contexts.contains_key(&key) && self.contexts.keys().filter(|(context_issi, _)| *context_issi == issi).count() >= 4 {
             self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 19), None);
             return Err("maximum of four PDP contexts per ISSI exceeded".into());
         }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:156:
                     return Err(format!("invalid static IPv4 address {address:?}"));
                 }
                 let in_use = self.contexts.iter().any(|(&(context_issi, context_nsapi), context)| {
-                    (context_issi, context_nsapi) != (ind.received_tetra_address.ssi, demand.nsapi)
-                        && context.address == address
+                    (context_issi, context_nsapi) != (ind.received_tetra_address.ssi, demand.nsapi) && context.address == address
                 });
                 if in_use {
                     self.send_control(queue, ind, encode_activate_reject(demand.nsapi, 9), None);
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:197:
         Ok(())
     }
 
-    fn handle_deactivate(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_deactivate(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let mut reader = reader(pdu);
         expect(&mut reader, 4, SN_DEACTIVATE_DEMAND as u64, "deactivate type")?;
         let selector = read(&mut reader, 8, "deactivation selector")? as u8;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:232:
         Ok(())
     }
 
-    fn handle_data_transmit(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_data_transmit(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let mut reader = reader(pdu);
         expect(&mut reader, 4, SN_DATA_TRANSMIT_REQUEST as u64, "data transmit type")?;
         let nsapi = read(&mut reader, 4, "NSAPI")? as u8;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:250:
         context.state = PacketState::Ready;
         let response = encode_data_transmit_response(nsapi, true, None);
         self.send_control(queue, ind, response, Some(packet_data_channel()));
-        tracing::info!("SNDCP/WAP: ISSI={} NSAPI={} entered READY on TS2", ind.received_tetra_address.ssi, nsapi);
+        tracing::info!(
+            "SNDCP/WAP: ISSI={} NSAPI={} entered READY on TS2",
+            ind.received_tetra_address.ssi,
+            nsapi
+        );
         Ok(())
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:257:
-    fn handle_reconnect(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_reconnect(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let mut reader = reader(pdu);
         expect(&mut reader, 4, SN_RECONNECT as u64, "reconnect type")?;
         let has_data = read(&mut reader, 1, "data to send")? != 0;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:282:
         }
     }
 
-    fn handle_end_of_data(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_end_of_data(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &BitBuffer) -> Result<(), String> {
         let mut reader = reader(pdu);
         expect(&mut reader, 4, SN_END_OF_DATA as u64, "end-of-data type")?;
         let immediate = read(&mut reader, 1, "immediate service change")?;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:307:
         Ok(())
     }
 
-    fn handle_user_data(
-        &mut self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        pdu: &mut BitBuffer,
-    ) -> Result<(), String> {
+    fn handle_user_data(&mut self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, pdu: &mut BitBuffer) -> Result<(), String> {
         if !self.wap.enabled {
             return Err("WAP/IP endpoint is disabled".into());
         }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:332:
             .ok_or_else(|| "truncated N-PDU".to_string())?;
 
         let key = (ind.received_tetra_address.ssi, nsapi);
-        let context = self.contexts.get(&key).copied().ok_or_else(|| format!("missing PDP context for NSAPI {nsapi}"))?;
+        let context = self
+            .contexts
+            .get(&key)
+            .copied()
+            .ok_or_else(|| format!("missing PDP context for NSAPI {nsapi}"))?;
         let request_source = npdu.get(12..16).ok_or_else(|| "IPv4 N-PDU too short".to_string())?;
         if request_source != &context.address[..] {
             return Err(format!(
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/sndcp_bs.rs:411:
         }
     }
 
-    fn send_control(
-        &self,
-        queue: &mut MessageQueue,
-        ind: &LtpdMleUnitdataInd,
-        sn_pdu: BitBuffer,
-        chan_alloc: Option<CmceChanAllocReq>,
-    ) {
+    fn send_control(&self, queue: &mut MessageQueue, ind: &LtpdMleUnitdataInd, sn_pdu: BitBuffer, chan_alloc: Option<CmceChanAllocReq>) {
         queue.push_back(SapMsg {
             sap: Sap::TlaSap,
             src: TetraEntity::Sndcp,
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:87:
 }
 
 /// Build a complete IPv4/UDP response. `Ok(None)` means the inbound WTP PDU was an ACK/ABORT.
-pub fn build_response(
-    request_npdu: &[u8],
-    endpoint: WapEndpoint,
-    snapshot: &WapStatusSnapshot,
-) -> Result<Option<Vec<u8>>, WapError> {
+pub fn build_response(request_npdu: &[u8], endpoint: WapEndpoint, snapshot: &WapStatusSnapshot) -> Result<Option<Vec<u8>>, WapError> {
     let ip = parse_ipv4_packet(request_npdu)?;
     if ip.fragmented {
         return Err(WapError::Fragmented);
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:212:
                 let len = *payload.get(offset + 1).ok_or(WapError::MalformedWtpWsp)? as usize;
                 offset = offset.checked_add(2 + len).ok_or(WapError::MalformedWtpWsp)?;
             } else {
-                offset = offset
-                    .checked_add(1 + (h & 0x03) as usize)
-                    .ok_or(WapError::MalformedWtpWsp)?;
+                offset = offset.checked_add(1 + (h & 0x03) as usize).ok_or(WapError::MalformedWtpWsp)?;
             }
             if offset > payload.len() {
                 return Err(WapError::MalformedWtpWsp);
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:338:
     let state = escape(&snapshot.service_state, 12);
     let uptime = compact_uptime(snapshot.uptime_secs);
     let body = match sector {
-        0 => format!("{title}<br/>{state} MS:{} C:{}<br/>Up:{uptime}<br/><a href=\"?s=1\">N</a>", snapshot.registered_ms, snapshot.active_calls),
-        1 => format!("Net {}/{}<br/>Carrier:{}<br/>SDS:{}<br/><a href=\"?s=2\">N</a> <a href=\"?s=0\">H</a>", snapshot.mcc, snapshot.mnc, snapshot.carrier, snapshot.queued_sds),
+        0 => format!(
+            "{title}<br/>{state} MS:{} C:{}<br/>Up:{uptime}<br/><a href=\"?s=1\">N</a>",
+            snapshot.registered_ms, snapshot.active_calls
+        ),
+        1 => format!(
+            "Net {}/{}<br/>Carrier:{}<br/>SDS:{}<br/><a href=\"?s=2\">N</a> <a href=\"?s=0\">H</a>",
+            snapshot.mcc, snapshot.mnc, snapshot.carrier, snapshot.queued_sds
+        ),
         _ => format!("Packet data OK<br/>WTP/WSP active<br/>UDP 9200<br/><a href=\"?s=0\">H</a>"),
     };
     let candidates = match format {
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:346:
         PageFormat::Xhtml => vec![
             format!("<html><body>{body}</body></html>"),
-            format!("<html><body>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></body></html>", snapshot.registered_ms),
+            format!(
+                "<html><body>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></body></html>",
+                snapshot.registered_ms
+            ),
             format!("<html><body>{title}<br/>{state}</body></html>"),
         ],
         PageFormat::Wml => vec![
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:352:
             format!("<wml><card><p>{body}</p></card></wml>"),
-            format!("<wml><card><p>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></p></card></wml>", snapshot.registered_ms),
+            format!(
+                "<wml><card><p>{title}<br/>{state} MS:{}<br/><a href=\"?s=1\">N</a></p></card></wml>",
+                snapshot.registered_ms
+            ),
             format!("<wml><card><p>{title}<br/>{state}</p></card></wml>"),
         ],
     };
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:437:
     #[test]
     fn connect_reply_matches_openwave_shape() {
         let caps = vec![
-            Capability { id: 0x80, value: vec![0x94, 0x80, 0x00] },
-            Capability { id: 0x81, value: vec![0x94, 0x80, 0x00] },
+            Capability {
+                id: 0x80,
+                value: vec![0x94, 0x80, 0x00],
+            },
+            Capability {
+                id: 0x81,
+                value: vec![0x94, 0x80, 0x00],
+            },
         ];
         assert_eq!(
             build_wtp_result(0x13cc, &build_connect_reply(&caps)),
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:445:
-            vec![0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21]
+            vec![
+                0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21
+            ]
         );
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:454:
     #[test]
     fn complete_ipv4_udp_connect_roundtrip_is_byte_exact() {
         let request_payload = vec![
-            0x08, 0x13, 0xcc, 0x12,
-            0x01, 0x10, 0x08, 0x00,
-            0x03, 0x80, 0x84, 0x21,
-            0x03, 0x81, 0x84, 0x21,
+            0x08, 0x13, 0xcc, 0x12, 0x01, 0x10, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21,
         ];
-        let request = build_ipv4_udp_npdu(
-            [10, 0, 0, 2],
-            [10, 0, 0, 1],
-            49_152,
-            9_200,
-            &request_payload,
-            0x2222,
-            64,
-        )
-        .unwrap();
+        let request = build_ipv4_udp_npdu([10, 0, 0, 2], [10, 0, 0, 1], 49_152, 9_200, &request_payload, 0x2222, 64).unwrap();
         let snapshot = WapStatusSnapshot {
             title: "NetCore-TETRA".into(),
             service_state: "ON AIR".into(),
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/sndcp/wap.rs:503:
         assert_eq!(udp.destination_port, 49_152);
         assert_eq!(
             udp.payload,
-            &[0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21]
+            &[
+                0x12, 0x93, 0xcc, 0x02, 0x01, 0x08, 0x00, 0x03, 0x80, 0x84, 0x21, 0x03, 0x81, 0x84, 0x21
+            ]
         );
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:55:
     TrafficOnly,
 }
 
-
 /// Number of timeslots the scheduler operates on. May become larger when secondary carriers are supported.
 pub const NUM_TIMESLOTS: usize = 4;
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:586:
                     }
                     if let Some(previous) = self.packet_data_assignments[slot] {
                         if !Self::same_address(previous.addr, addr) {
-                            tracing::info!(
-                                "packet-data PDCH TS2 owner changed {} -> {}",
-                                previous.addr,
-                                addr
-                            );
+                            tracing::info!("packet-data PDCH TS2 owner changed {} -> {}", previous.addr, addr);
                         }
                     }
                     self.packet_data_assignments[slot] = Some(PacketDataAssignment {
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:598:
                         ul_dl_assigned,
                         expires_at: self.cur_dltime.add_timeslots(PACKET_DATA_ASSIGNMENT_TTL_TIMESLOTS),
                     });
-                    tracing::info!(
-                        "packet-data PDCH assigned to {} on TS2 ({})",
-                        addr,
-                        ul_dl_assigned
-                    );
+                    tracing::info!("packet-data PDCH assigned to {} on TS2 ({})", addr, ul_dl_assigned);
                 } else {
-                    self.packet_data_assignments[1] = self.packet_data_assignments[1]
-                        .filter(|current| !Self::same_address(current.addr, addr));
+                    self.packet_data_assignments[1] =
+                        self.packet_data_assignments[1].filter(|current| !Self::same_address(current.addr, addr));
                 }
             }
             ChanAllocType::ReplaceWithCarrierSignalling => {
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:619:
             return None;
         }
         self.packet_data_assignments[1].filter(|assignment| {
-            assignment.expires_at.diff(ts) > 0
-                && matches!(assignment.ul_dl_assigned, UlDlAssignment::Ul | UlDlAssignment::Both)
+            assignment.expires_at.diff(ts) > 0 && matches!(assignment.ul_dl_assigned, UlDlAssignment::Ul | UlDlAssignment::Both)
         })
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:673:
                 } else {
                     AccessAssignUlUsage::CommonOnly
                 }
-            },
+            }
             _ => unreachable!("ul2 can't be set with ul1 None"),
         }
     }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:702:
     /// Total queued downlink scheduling elements across all timeslots plus the next-slot carry-over.
     /// A cheap backlog gauge for the health monitor's Congestion domain (read once per tick).
     pub fn dl_queue_depth(&self) -> usize {
-        self.dltx_queues.iter().map(|q| q.len()).sum::<usize>()
-            + self.dltx_next_slot_queue.len()
-            + self.frame18_common_scch_queue.len()
+        self.dltx_queues.iter().map(|q| q.len()).sum::<usize>() + self.dltx_next_slot_queue.len() + self.frame18_common_scch_queue.len()
     }
 
     /// Registers that we should transmit a MAC-RESOURCE or similar with a grant, somewhere this tick.
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:834:
     /// Only one pending page per call/GSSI is retained. A newer call to the same
     /// GSSI supersedes queued pages from the old call, preventing a later frame-18
     /// opportunity from announcing an already released call or an obsolete usage marker.
-    pub fn dl_enqueue_frame18_common_scch(
-        &mut self,
-        call_id: u16,
-        pdu: MacResource,
-        sdu: BitBuffer,
-        tx_reporter: Option<TxReporter>,
-    ) {
+    pub fn dl_enqueue_frame18_common_scch(&mut self, call_id: u16, pdu: MacResource, sdu: BitBuffer, tx_reporter: Option<TxReporter>) {
         let Some(addr) = pdu.addr else {
             tracing::warn!(
                 "BsChannelScheduler: dropping frame-18 common SCCH resource without address call_id={}",
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:904:
     /// UMAC to retire the queue entry before a later call can inherit it.
     pub fn drop_frame18_common_scch_call(&mut self, call_id: u16) {
         let before = self.frame18_common_scch_queue.len();
-        self.frame18_common_scch_queue
-            .retain(|entry| entry.call_id != call_id);
+        self.frame18_common_scch_queue.retain(|entry| entry.call_id != call_id);
         let dropped = before - self.frame18_common_scch_queue.len();
         if dropped > 0 {
             tracing::info!(
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:1531:
         // During hangtime we stop sending traffic frames and switch to signalling mode.
         // Keep traffic mode while FACCH/stealing is still queued for delivery.
         let hang_slot = (2..=4).contains(&ts.t) || (self.downlink_mode == CarrierDownlinkMode::TrafficOnly && ts.t == 1);
-        let hang_effective = if hang_slot {
-            self.is_hangtime_effective(ts.t)
-        } else {
-            false
-        };
+        let hang_effective = if hang_slot { self.is_hangtime_effective(ts.t) } else { false };
 
         let dl_is_traffic = dl_circuit_active && !hang_effective;
         let ul_is_traffic = ul_circuit_active && !hang_effective;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:1547:
         // Returning an empty TP slot makes PHY skip transmission for this carrier/slot.
         if ((self.downlink_mode == CarrierDownlinkMode::TrafficOnly)
             || (self.downlink_mode == CarrierDownlinkMode::SecondaryBcchNoMcch && ts.t != 1))
-            && !dl_is_traffic && !ul_is_traffic {
+            && !dl_is_traffic
+            && !ul_is_traffic
+        {
             let clear_ts = ts.add_timeslots(-4);
             let index = self.ul_ts_to_sched_index(&clear_ts);
             self.ulsched[ts.t as usize - 1][index].ul1 = None;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:1636:
                 // Otherwise, fall back to default SYNC/SYSINFO.
                 if hang_effective && dl_circuit_active {
                     TmvUnitdataReqSlot {
-                    carrier_num: self.carrier_num,
+                        carrier_num: self.carrier_num,
                         ts,
                         blk1: Some(TmvUnitdataReq {
                             logical_channel: LogicalChannel::SchF,
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:1650:
                 } else {
                     // Put default SYNC/SYSINFO frame
                     TmvUnitdataReqSlot {
-                    carrier_num: self.carrier_num,
+                        carrier_num: self.carrier_num,
                         ts,
                         blk1: None,
                         blk2: None,
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:2594:
         };
         let ts2 = TdmaTime { t: 2, f: 2, m: 1, h: 0 };
 
-        sched.update_packet_data_assignment(
-            addr,
-            [false, true, false, false],
-            ChanAllocType::Replace,
-            UlDlAssignment::Both,
-        );
+        sched.update_packet_data_assignment(addr, [false, true, false, false], ChanAllocType::Replace, UlDlAssignment::Both);
 
-        assert_eq!(sched.packet_data_uplink_owner(ts2, PhyBlockNum::Both).map(|owner| owner.ssi), Some(addr.ssi));
+        assert_eq!(
+            sched.packet_data_uplink_owner(ts2, PhyBlockNum::Both).map(|owner| owner.ssi),
+            Some(addr.ssi)
+        );
         assert_eq!(sched.packet_data_downlink_timeslot_for(addr), Some(2));
         assert_eq!(sched.ul_get_usage(ts2), AccessAssignUlUsage::AssignedOnly);
-        assert!(sched
-            .packet_data_uplink_owner(TdmaTime { f: 18, ..ts2 }, PhyBlockNum::Both)
-            .is_none());
-
-        sched.update_packet_data_assignment(
-            addr,
-            [false; 4],
-            ChanAllocType::QuitAndGo,
-            UlDlAssignment::Both,
+        assert!(
+            sched
+                .packet_data_uplink_owner(TdmaTime { f: 18, ..ts2 }, PhyBlockNum::Both)
+                .is_none()
         );
+
+        sched.update_packet_data_assignment(addr, [false; 4], ChanAllocType::QuitAndGo, UlDlAssignment::Both);
         assert!(sched.packet_data_uplink_owner(ts2, PhyBlockNum::Both).is_none());
         assert_eq!(sched.packet_data_downlink_timeslot_for(addr), None);
     }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/subcomp/bs_sched.rs:2620:
-
 }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:29:
 use tetra_saps::lcmc::enums::alloc_type::ChanAllocType;
 use tetra_saps::lcmc::enums::ul_dl_assignment::UlDlAssignment;
 use tetra_saps::lcmc::fields::chan_alloc_req::CmceChanAllocReq;
-use tetra_saps::tma::{
-    parse_frame18_common_scch_handle, TmaReport, TmaReportInd, TmaUnitdataInd, TmaUnitdataReq,
-};
-use tetra_saps::tmv::{TmvConfigureReq, TmvUnitdataReqSlots};
+use tetra_saps::tma::{TmaReport, TmaReportInd, TmaUnitdataInd, TmaUnitdataReq, parse_frame18_common_scch_handle};
 use tetra_saps::tmv::enums::logical_chans::LogicalChannel;
+use tetra_saps::tmv::{TmvConfigureReq, TmvUnitdataReqSlots};
 use tetra_saps::{SapMsg, SapMsgInner};
 
 use crate::lmac::components::scrambler;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:144:
         {
             return &mut self.secondary_channel_schedulers[index];
         }
-        tracing::error!("UMAC: unknown carrier {}, no scheduler configured -- falling back to primary", carrier_num);
+        tracing::error!(
+            "UMAC: unknown carrier {}, no scheduler configured -- falling back to primary",
+            carrier_num
+        );
         &mut self.channel_scheduler
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:159:
         {
             Some(sched) => sched,
             None => {
-                tracing::error!("UMAC: unknown carrier {}, no scheduler configured -- falling back to primary", carrier_num);
+                tracing::error!(
+                    "UMAC: unknown carrier {}, no scheduler configured -- falling back to primary",
+                    carrier_num
+                );
                 &self.channel_scheduler
             }
         }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:206:
             }),
             Some(other) => {
                 tracing::warn!("UMAC: ignoring unknown negative CMCE carrier hint {}", other);
-                logical_ts.map(|ts| self.carrier_for_logical_ts(ts)).unwrap_or_else(|| self.main_carrier())
+                logical_ts
+                    .map(|ts| self.carrier_for_logical_ts(ts))
+                    .unwrap_or_else(|| self.main_carrier())
             }
-            None => logical_ts.map(|ts| self.carrier_for_logical_ts(ts)).unwrap_or_else(|| self.main_carrier()),
+            None => logical_ts
+                .map(|ts| self.carrier_for_logical_ts(ts))
+                .unwrap_or_else(|| self.main_carrier()),
         }
     }
 
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:643:
                     self.packet_data_link_contexts.clear();
                 }
                 if chan_alloc.timeslots.iter().any(|assigned| *assigned) {
-                    self.packet_data_link_contexts
-                        .insert(prim.main_address.ssi, prim.endpoint_id);
+                    self.packet_data_link_contexts.insert(prim.main_address.ssi, prim.endpoint_id);
                 }
             }
             ChanAllocType::ReplaceWithCarrierSignalling => {}
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:683:
         let msg_dltime = self.dltime.add_timeslots(-2); // Msg on uplink was sent two timeslots ago.
         let carrier_num = prim.carrier_num;
         let logical_ts = self.logical_ts_for_carrier_air_ts(carrier_num, msg_dltime.t);
-        let logical_dltime = TdmaTime { t: logical_ts, ..msg_dltime };
-        let assigned_owner = self
-            .scheduler_for(carrier_num)
-            .packet_data_uplink_owner(msg_dltime, prim.block_num);
+        let logical_dltime = TdmaTime {
+            t: logical_ts,
+            ..msg_dltime
+        };
+        let assigned_owner = self.scheduler_for(carrier_num).packet_data_uplink_owner(msg_dltime, prim.block_num);
         let Some(addr) = pdu.addr.or(assigned_owner) else {
             tracing::warn!("UMAC: rx_mac_data: PDU has no address and no assigned PDCH owner; dropping");
             return;
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:780:
             if let Some((grant, usage_marker)) = grant_result {
                 // Schedule grant — marker propagates into the MAC-RESOURCE ACK
                 // so the MS can tag its reservation when continuing the burst.
-                self.scheduler_for_mut(carrier_num).dl_enqueue_grant(msg_dltime.t, addr, grant, usage_marker);
+                self.scheduler_for_mut(carrier_num)
+                    .dl_enqueue_grant(msg_dltime.t, addr, grant, usage_marker);
             } else {
                 tracing::warn!("rx_mac_data: No grant for reservation request {:?}", res_req);
             }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:926:
         let msg_dltime = self.dltime.add_timeslots(-2); // Msg on uplink was sent two timeslots ago.
         let carrier_num = prim.carrier_num;
         let logical_ts = self.logical_ts_for_carrier_air_ts(carrier_num, msg_dltime.t);
-        let logical_dltime = TdmaTime { t: logical_ts, ..msg_dltime };
+        let logical_dltime = TdmaTime {
+            t: logical_ts,
+            ..msg_dltime
+        };
         if !self.scheduler_for(carrier_num).circuit_is_active(Direction::Dl, msg_dltime.t) {
             self.scheduler_for_mut(carrier_num).dl_enqueue_random_access_ack(msg_dltime.t, addr);
         } else {
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:964:
             if let Some((grant, usage_marker)) = grant_result {
                 // Schedule grant — marker propagates into the MAC-RESOURCE ACK
                 // so the MS can tag its reservation when continuing the burst.
-                self.scheduler_for_mut(carrier_num).dl_enqueue_grant(msg_dltime.t, addr, grant, usage_marker);
+                self.scheduler_for_mut(carrier_num)
+                    .dl_enqueue_grant(msg_dltime.t, addr, grant, usage_marker);
             } else {
                 tracing::warn!("rx_mac_access: No grant for reservation request {:?}", res_req);
             }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1065:
         let msg_dltime = self.dltime.add_timeslots(-2); // Msg on uplink was sent two timeslots ago.
         let carrier_num = prim.carrier_num;
         let logical_ts = self.logical_ts_for_carrier_air_ts(carrier_num, msg_dltime.t);
-        let logical_dltime = TdmaTime { t: logical_ts, ..msg_dltime };
+        let logical_dltime = TdmaTime {
+            t: logical_ts,
+            ..msg_dltime
+        };
         let Some(slot_owner) = self
             .scheduler_for(carrier_num)
             .ul_get_slot_owner(msg_dltime, prim.block_num)
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1150:
         let msg_dltime = self.dltime.add_timeslots(-2); // Msg on uplink was sent two timeslots ago.
         let carrier_num = prim.carrier_num;
         let logical_ts = self.logical_ts_for_carrier_air_ts(carrier_num, msg_dltime.t);
-        let logical_dltime = TdmaTime { t: logical_ts, ..msg_dltime };
+        let logical_dltime = TdmaTime {
+            t: logical_ts,
+            ..msg_dltime
+        };
         let Some(slot_owner) = self
             .scheduler_for(carrier_num)
             .ul_get_slot_owner(msg_dltime, prim.block_num)
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1183:
 
         // Handle reservation if present
         if let Some(res_req) = &pdu.reservation_req {
-            let grant_result = self.scheduler_for_mut(carrier_num).ul_process_cap_req(msg_dltime.t, defragbuf.addr, res_req);
+            let grant_result = self
+                .scheduler_for_mut(carrier_num)
+                .ul_process_cap_req(msg_dltime.t, defragbuf.addr, res_req);
             if let Some((grant, usage_marker)) = grant_result {
                 // Schedule grant — marker propagates into the MAC-RESOURCE ACK
                 // so the MS can tag its reservation when continuing the burst.
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1287:
         let msg_dltime = self.dltime.add_timeslots(-2); // Msg on uplink was sent two timeslots ago.
         let carrier_num = prim.carrier_num;
         let logical_ts = self.logical_ts_for_carrier_air_ts(carrier_num, msg_dltime.t);
-        let logical_dltime = TdmaTime { t: logical_ts, ..msg_dltime };
+        let logical_dltime = TdmaTime {
+            t: logical_ts,
+            ..msg_dltime
+        };
         let Some(slot_owner) = self.scheduler_for(carrier_num).ul_get_slot_owner(msg_dltime, prim.block_num) else {
             tracing::debug!(
                 "rx_mac_end_hu: MAC-END-HU for unassigned block {:?} on carrier {} logical ts {} / air ts {} (start not seen — normal on RF loss)",
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1312:
 
         // Handle reservation if present
         if let Some(res_req) = &pdu.reservation_req {
-            let grant_result = self.scheduler_for_mut(carrier_num).ul_process_cap_req(msg_dltime.t, defragbuf.addr, res_req);
+            let grant_result = self
+                .scheduler_for_mut(carrier_num)
+                .ul_process_cap_req(msg_dltime.t, defragbuf.addr, res_req);
             if let Some((grant, usage_marker)) = grant_result {
                 // Schedule grant — marker propagates into the MAC-RESOURCE ACK
                 // so the MS can tag its reservation when continuing the burst.
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1502:
                     // Build MAC-RESOURCE PDU for the STCH half-slot (124 type1 bits).
                     const STCH_CAP: usize = 124;
 
-                    let has_pending_ra = self.scheduler_for_mut(carrier_num).take_pending_ra_ack(air_ts, prim.main_address.ssi);
+                    let has_pending_ra = self
+                        .scheduler_for_mut(carrier_num)
+                        .take_pending_ra_ack(air_ts, prim.main_address.ssi);
                     // FACCH/STCH on an already allocated traffic slot is not a random-access
                     // response by default. Only propagate the flag when we are actually
                     // carrying a pending RA acknowledgement on this same timeslot.
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1548:
                             stch_block.get_len()
                         );
 
-                        self.scheduler_for_mut(carrier_num).dl_enqueue_stealing(air_ts, stch_block, prim.tx_reporter);
+                        self.scheduler_for_mut(carrier_num)
+                            .dl_enqueue_stealing(air_ts, stch_block, prim.tx_reporter);
                     } else {
                         // Larger than one stolen half-slot: fragment across consecutive stolen
                         // half-slots (panic-safe — a fixed 124-bit buffer used to overflow here and
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1582:
         let (usage_marker, mac_chan_alloc) = if let Some(chan_alloc) = prim.chan_alloc {
             let logical_ts = self.first_logical_ts_in_chan_alloc(&chan_alloc);
             let target_carrier = self.resolve_cmce_carrier_hint(chan_alloc.carrier, logical_ts);
-            (
-                chan_alloc.usage,
-                Some(Self::cmce_to_mac_chanalloc(&chan_alloc, target_carrier)),
-            )
+            (chan_alloc.usage, Some(Self::cmce_to_mac_chanalloc(&chan_alloc, target_carrier)))
         } else {
             (None, None)
         };
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1628:
                 match self.channel_scheduler.packet_data_downlink_timeslot_for(prim.main_address) {
                     Some(ts) => u32::from(ts),
                     None => {
-                        tracing::warn!(
-                            "SNDCP downlink for {} has no active PDCH; falling back to MCCH",
-                            prim.main_address
-                        );
+                        tracing::warn!("SNDCP downlink for {} has no active PDCH; falling back to MCCH", prim.main_address);
                         0
                     }
                 }
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/src/umac/umac_bs.rs:1846:
 
                 let dl_target_carrier = self.carrier_for_logical_ts(dl_target_logical_ts);
                 let dl_target_air_ts = Self::air_ts_for_logical(dl_target_logical_ts);
-                if self.scheduler_for(dl_target_carrier).circuit_is_active(Direction::Dl, dl_target_air_ts) {
+                if self
+                    .scheduler_for(dl_target_carrier)
+                    .circuit_is_active(Direction::Dl, dl_target_air_ts)
+                {
                     if let Some(packed) = pack_ul_acelp_bits(&data) {
                         self.scheduler_for_mut(dl_target_carrier).dl_schedule_tmd(dl_target_air_ts, packed);
                     } else {
Diff in /home/runner/work/netcore-tetra/netcore-tetra/crates/tetra-entities/tests/common/default_stack.rs:1:
 use tetra_config::bluestation::{
-    CfgAsterisk, CfgAudioPlayer, CfgCellInfo, CfgWapIp, CfgDapnet, CfgEcholink, CfgEmergency, CfgGeoalarm, CfgHealth, CfgMeshcom, CfgNetInfo, CfgPhyIo, CfgRecording, CfgRecovery,
-    CfgSecurity, CfgSnomNotify, CfgTpg2200Action, CfgTts, CfgWxService, PhyBackend, StackConfig, StackMode,
+    CfgAsterisk, CfgAudioPlayer, CfgCellInfo, CfgDapnet, CfgEcholink, CfgEmergency, CfgGeoalarm, CfgHealth, CfgMeshcom, CfgNetInfo,
+    CfgPhyIo, CfgRecording, CfgRecovery, CfgSecurity, CfgSnomNotify, CfgTpg2200Action, CfgTts, CfgWapIp, CfgWxService, PhyBackend,
+    StackConfig, StackMode,
 };
 use tetra_core::{freqs::FreqInfo, ranges::SortedDisjointSsiRanges};
 
```
