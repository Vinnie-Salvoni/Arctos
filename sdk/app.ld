ENTRY(app_entry)

SECTIONS {
    . = 0x400000000000;
    
    .text : {
        *(.text.app_entry)
        *(.text*)
    }
    
    .rodata : {
        *(.rodata*)
    }
    
    .data : {
        *(.data*)
    }
    
    .bss : {
        *(.bss*)
    }
}
