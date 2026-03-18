version 1.0

workflow Echo {
  input {
    String message
  }

  call EchoTask { input: message = message }

  output {
    String echo_out = EchoTask.echo_out
  }
}

task EchoTask {
  input {
    String message
  }

  command <<<
    echo ~{message} > wdl_echo_out.txt
  >>>

  output {
    String echo_out = read_string("wdl_echo_out.txt")
  }

  runtime {
    docker: "alpine:3.18"
  }
}

