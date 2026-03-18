#!/usr/bin/env nextflow

nextflow.enable.dsl=2

params.message = params.message ?: 'hello-nextflow_echo'

process Echo {
    input:
      val msg from params.message

    output:
      path 'nextflow_echo_out.txt'

    """
    echo "$msg" > nextflow_echo_out.txt
    """
}

workflow {
    Echo(params.message)
}

