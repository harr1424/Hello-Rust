# ferris-files

```
A CLI to find large files

Usage: ferris-files [OPTIONS]

Options:
  -n, --num_entries <NUM_ENTRIES>
          (optional) Number of largest entries to output [default: 10]
  -b, --batch_Size <BATCH_SIZE>
          (optional) Number of files to size at one time [default: 1000]
  -d, --directory <TARGET_DIR>
          (optional) defaults to attempting to detect current working directory
  -x, --excluded-dirs-file <EXCLUSION_FILE>
          (optional) Path to a file where each line specifies a directory to ignore
  -v, --verbose
          
  -h, --help
          Print help
  -V, --version
          Print version
```

```
Preparing to scan using 12 threads
Limiting open file handles to 524287
Searching for 10 largest entries in /Users/user:
  [00:00:15] Directory scan complete (121 errors encountered: run with -v for details)
  [00:00:15] Processed 1399886 files (1399886 successful, 0 failed)                                                                                                                                                                                                                                        

/Users/user/Movies/TV/Media.localized/Movies/Twin Peaks_ Fire Walk with Me/Twin Peaks_ Fire Walk with Me (1080p HD).m4v: 5.17 GB
/Users/user/Library/Android/sdk/system-images/android-33/google_apis/x86_64/system.img: 4.01 GB
/Users/user/Library/Android/sdk/system-images/android-34/google_apis/x86_64/system.img: 4.01 GB
/Users/user/Library/Android/sdk/system-images/android-31/google_apis/x86_64/system.img: 4.01 GB
/Users/user/Movies/TV/Media.localized/Movies/The Animatrix/04 The Animatrix (1080p HD).m4v: 3.65 GB
/Users/user/.android/avd/Pixel_5_API_33.avd/userdata-qemu.img.qcow2: 3.54 GB
/Users/user/Library/Android/sdk/system-images/android-30/google_apis/x86/system.img: 3.01 GB
/Users/user/Library/Android/sdk/system-images/android-31/google_apis_playstore/x86_64/system.img: 2.67 GB
/Users/user/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw: 2.32 GB
/Users/user/Virtual Machines.localized/kali-linux-2024.2-vmware-amd64.vmwarevm/kali-linux-2024.2-vmware-amd64-s035.vmdk: 1.97 GB

Program completed in 15.199807 seconds
```

I created the same program using [C++](https://github.com/harr1424/cpp_filesystem_size), [C](https://github.com/harr1424/c_filesystem_size), and [Go](https://github.com/harr1424/go_filesystem_size). 
