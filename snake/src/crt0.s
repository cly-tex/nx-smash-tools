
.section ".text.crt0","ax"
.global __module_start
__module_start:
    .word 0
    .word __nx_mod0 - __module_start
    .word __sdk_version - __module_start
    .align 4
    .ascii "snake           "

.global __custom_init
__custom_init:
    b main

.section ".rotdata.mod0","a"
.hidden snake_module_object
.align 2
__nx_mod0:
    .ascii "MOD0"
    .word  __dynamic_start__            - __nx_mod0
    .word  __bss_start__                - __nx_mod0
    .word  __bss_end__                  - __nx_mod0
    .word  __eh_frame_hdr_start__       - __nx_mod0
    .word  __eh_frame_hdr_end__         - __nx_mod0
    .word  snake_module_object          - __nx_mod0
    .word   __relro_start__             - __nx_mod0
    .word   __full_relro_end__          - __nx_mod0
    .word   __module_name_start__       - __nx_mod0
    .word   __module_name_end__         - __nx_mod0
    .word   __note_gnu_build_id_start__ - __nx_mod0
    .word   __note_gnu_build_id_end__   - __nx_mod0

__sdk_version:
    .word 20
    .word 5
    .word 6
