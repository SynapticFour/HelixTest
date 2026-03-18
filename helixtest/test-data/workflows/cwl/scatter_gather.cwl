class: Workflow
cwlVersion: v1.2

inputs:
  items:
    type:
      type: array
      items: int

outputs:
  scatter_result:
    type: File
    outputSource: gather/out_file

steps:
  scatter:
    run:
      class: CommandLineTool
      baseCommand: bash
      inputs:
        item:
          type: int
          inputBinding:
            position: 1
      outputs:
        out_file:
          type: File
          outputBinding:
            glob: "$(inputs.item).txt"
      arguments:
        - valueFrom: |
            echo $(inputs.item) > $(inputs.item).txt
          position: 2
    in:
      item: items
    out: [out_file]
    scatter: item
    scatterMethod: dotproduct

  gather:
    run:
      class: CommandLineTool
      baseCommand: bash
      inputs:
        infiles:
          type:
            type: array
            items: File
          inputBinding:
            position: 1
            prefix: ""
      outputs:
        out_file:
          type: File
          outputBinding:
            glob: scatter_gather_out.txt
      arguments:
        - valueFrom: |
            cat $(inputs.infiles[*].basename) | tr '\n' ',' | sed 's/,$//' > scatter_gather_out.txt
          position: 2
    in:
      infiles: scatter/out_file
    out: [out_file]

