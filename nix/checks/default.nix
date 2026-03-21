{ vmLib }:
{
  vm-smoke = vmLib.mkVMCheck {
    name = "capsa";
    timeout = 60;
    expectProgram = ''
      expect {
        -re {~ # } {}
        timeout { puts "ERROR: timed out waiting for VM prompt"; exit 1 }
        eof { puts "ERROR: capsa exited before prompt"; exit 1 }
      }

      send -- "echo TTY_OK\r"
      expect {
        -re {TTY_OK} {}
        timeout { puts "ERROR: did not observe TTY echo marker"; exit 1 }
        eof { puts "ERROR: capsa exited while checking tty"; exit 1 }
      }

      send -- "exit\r"
      expect {
        eof {}
        timeout { puts "ERROR: timed out waiting for VM shutdown after exit"; exit 1 }
      }
    '';
  };
}
