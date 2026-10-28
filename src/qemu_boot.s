.section .multiboot,"a"
.align 4
.long 0x1BADB002
.long 0x00000003
.long -(0x1BADB002 + 0x00000003)

.section .bss
.align 4096
boot_pml4:
.skip 4096
boot_pdpt:
.skip 4096
boot_page_directory:
.skip 4096
.align 4
boot_multiboot_magic:
.long 0
boot_multiboot_info:
.long 0
.align 16
boot_stack:
.skip 16384
boot_stack_top:

.section .text.boot
.code32
.global _start
.type _start, @function
_start:
    cli
    movl %eax, boot_multiboot_magic
    movl %ebx, boot_multiboot_info
    movl $boot_stack_top, %esp

    movl $boot_pdpt, %eax
    orl $0x03, %eax
    movl %eax, boot_pml4
    movl $0, boot_pml4 + 4

    movl $boot_page_directory, %eax
    orl $0x03, %eax
    movl %eax, boot_pdpt
    movl $0, boot_pdpt + 4

    movl $boot_page_directory, %edi
    xorl %ecx, %ecx
    movl $0x83, %ebx
1:
    movl %ebx, (%edi, %ecx, 8)
    movl $0, 4(%edi, %ecx, 8)
    addl $0x200000, %ebx
    incl %ecx
    cmpl $512, %ecx
    jne 1b

    lgdt boot_gdt_pointer
    movl %cr4, %eax
    orl $0x20, %eax
    movl %eax, %cr4
    movl $boot_pml4, %eax
    movl %eax, %cr3

    movl $0xc0000080, %ecx
    rdmsr
    orl $0x100, %eax
    wrmsr

    movl %cr0, %eax
    orl $0x80000001, %eax
    movl %eax, %cr0
    ljmp $0x08, $long_mode_start

.align 8
boot_gdt:
    .quad 0
    .quad 0x00af9a000000ffff
    .quad 0x00cf92000000ffff
boot_gdt_end:
boot_gdt_pointer:
    .word boot_gdt_end - boot_gdt - 1
    .long boot_gdt

.code64
long_mode_start:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    leaq boot_stack_top(%rip), %rsp
    xorq %rbp, %rbp
    movl boot_multiboot_magic(%rip), %edi
    movl boot_multiboot_info(%rip), %esi
    call hyperos_kernel_main
2:
    hlt
    jmp 2b