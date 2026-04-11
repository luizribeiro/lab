{ vmLib, pkgs, capsaPackage, capsaPaths }:
let
  assets = vmLib.mkVMAssets { name = "capsa-daemon-netd-spawn-failure"; };
  capsa = "${capsaPackage}/libexec/capsa/capsa";
in
pkgs.runCommand "capsa-daemon-netd-spawn-failure-vm-check"
{
  nativeBuildInputs = [ pkgs.expect pkgs.coreutils ];
}
  ''
    set -euo pipefail

    netd_stub="$TMPDIR/capsa-netd-non-executable"
    printf 'not executable\n' > "$netd_stub"
    chmod 0644 "$netd_stub"
    export NETD_STUB="$netd_stub"

    cat > vm-check.expect <<'EOF'
    #!${pkgs.expect}/bin/expect
    set timeout 20

    set capsa "${capsa}"
    set kernel "${assets.kernelImage}"
    set initramfs "${assets.initramfsImage}"
    set kernel_cmdline {${assets.vmConfig.kernelCmdline}}
    set env(LIBKRUN_LIB_DIR) "${capsaPaths.libkrunLibDir}"
    set env(CAPSA_DISABLE_SANDBOX) 1
    set env(CAPSA_NETD_PATH) $env(NETD_STUB)

    spawn $capsa \
      --kernel $kernel \
      --initramfs $initramfs \
      --kernel-cmdline $kernel_cmdline \
      --vcpus ${toString assets.vmConfig.vcpus} \
      --memory-mib ${toString assets.vmConfig.memoryMiB} \
      --allow-host "*"

    proc fail {message} {
      puts "ERROR: $message"
      exit 1
    }

    set vm_prompt_re {~ # }

    expect {
      -re {failed to spawn network daemon} {}
      -re $vm_prompt_re { fail "unexpectedly reached VM prompt before netd spawn failure" }
      timeout { fail "timed out waiting for netd spawn failure output" }
      eof { fail "capsa exited before reporting netd spawn failure" }
    }

    expect {
      eof {}
      timeout { fail "timed out waiting for capsa to exit after netd spawn failure" }
    }

    set wait_result [wait]
    set status [lindex $wait_result 3]
    if {$status == 0} {
      fail "capsa unexpectedly exited with status 0"
    }
    EOF

    chmod +x vm-check.expect
    ./vm-check.expect

    touch $out
  ''
