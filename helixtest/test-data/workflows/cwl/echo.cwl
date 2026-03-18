class: CommandLineTool
cwlVersion: v1.2
baseCommand: echo
inputs:
  message:
    type: string
    inputBinding:
      position: 1
outputs:
  echo_out:
    type: string
    outputBinding:
      glob: cwl_echo_out.txt
      loadContents: true
      outputEval: $(self[0].contents.trim())
stdout: cwl_echo_out.txt

