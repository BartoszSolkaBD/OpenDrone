# TII's flight logs

The CSV files in this folder are trimmed and converted from the **Race Against the Machine** dataset by TII Racing, licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/):

> Michael Bosello, Davide Aguiari, Yvo Keuter, Enrico Pallotta, Sara Kiade, Gyordan Caminati, Flavio Pinzarrone, Junaid Halepota, Jacopo Panerati and Giovanni Pau, "Race Against the Machine: A Fully-Annotated, Open-Design Dataset of Autonomous and Piloted High-Speed Flight", *IEEE Robotics and Automation Letters* 9(4), 3799–3806, 2024. doi:[10.1109/LRA.2024.3371288](https://doi.org/10.1109/LRA.2024.3371288). Data: [github.com/tii-racing/drone-racing-dataset](https://github.com/tii-racing/drone-racing-dataset), release v3.0.0.

**What changed.** `cargo xtask tii-logs` (in `crates/xtask/src/flight_logs.rs`) made them from the autonomous flights `flight-01a-ellipse` and `flight-04a-ellipse`, from TII's 500 Hz files (`<flight>_500hz_freq_sync.csv`), by:

- **trimming** each flight to the short windows the Scenarios beside them play back, from 0.2 s to 0.6 s long;
- keeping only the **motor commands** (`thrust[0]` to `thrust[3]`, as percent), each times its trim from the flight's still hover between 3 s and 6 s, in `<window>-motors.csv`;
- keeping only the **accelerometer and the battery voltage** (`accel_x`, `accel_y`, `accel_z` and `vbat`), less the accelerometer's forward and sideways reading in that hover, in `<window>-recorded.csv`;
- renaming the columns and units to OpenDrone's, and starting each window's time at 0 s.

Each file says the same in its comment lines. The Scenarios' starting states take the motion capture's position, attitude and speed and the gyro at each window's start.

The dataset's own code is MIT-licensed; none of it is in OpenDrone. [`CREDITS.md`](../../../../CREDITS.md) lists every file here with its checksum, and [the data policy](../../../../docs/data-policy.md) says what data may enter the repo.
