use crate::machine_verify::{SafepointSite, verify_safepoint_sites};
use crate::{AllocationTarget, CodegenError, FLAG_CALL, MachineVerificationError, SafepointMap};

fn emit_x86(assembler: &mut ncl_asm_x86_64::Assembler, instruction: &ncl_asm_x86_64::Inst) -> bool {
    assembler.emit(instruction).is_ok()
}

fn emit_aarch64(
    assembler: &mut ncl_asm_aarch64::Assembler,
    instruction: &ncl_asm_aarch64::Inst,
) -> bool {
    assembler.emit(instruction).is_ok()
}

#[test]
fn x86_64_rejects_map_pc_after_a_later_instruction_boundary() {
    let mut assembler = ncl_asm_x86_64::Assembler::new();
    let start_offset = u32::try_from(assembler.bytes().len());
    assert!(start_offset.is_ok());
    assert!(emit_x86(
        &mut assembler,
        &ncl_asm_x86_64::Inst::CallReg(ncl_asm_x86_64::Reg::R11)
    ));
    let end_offset = u32::try_from(assembler.bytes().len());
    assert!(end_offset.is_ok());
    assert!(emit_x86(&mut assembler, &ncl_asm_x86_64::Inst::Nop(1)));
    assert!(emit_x86(&mut assembler, &ncl_asm_x86_64::Inst::Ret));
    let blob = assembler.finish();
    assert!(blob.is_ok());
    let Ok(blob) = blob else {
        return;
    };
    let (Ok(start_offset), Ok(end_offset)) = (start_offset, end_offset) else {
        return;
    };
    let map = SafepointMap::new(end_offset, 4, 4, &[], &[], FLAG_CALL);
    assert!(map.is_ok());
    let Ok(map) = map else {
        return;
    };
    let sites = [SafepointSite::call(start_offset, end_offset)];
    assert_eq!(
        verify_safepoint_sites(
            AllocationTarget::X86_64,
            &blob.bytes,
            std::slice::from_ref(&map),
            &sites
        ),
        Ok(())
    );

    let mut corrupted_map = map;
    corrupted_map.pc_offset = end_offset.saturating_add(1);
    assert!(corrupted_map.pc_offset > end_offset);
    assert!(usize::try_from(corrupted_map.pc_offset).is_ok_and(|pc| pc < blob.bytes.len()));
    assert_eq!(
        verify_safepoint_sites(
            AllocationTarget::X86_64,
            &blob.bytes,
            &[corrupted_map],
            &sites,
        ),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::MachineSafepointPcMismatch,
        ))
    );
}

#[test]
fn aarch64_rejects_map_pc_after_a_later_instruction_boundary() {
    let mut assembler = ncl_asm_aarch64::Assembler::new();
    let start_offset = u32::try_from(assembler.offset());
    assert!(start_offset.is_ok());
    assert!(emit_aarch64(
        &mut assembler,
        &ncl_asm_aarch64::Inst::Blr {
            rn: ncl_asm_aarch64::Reg(17),
        }
    ));
    let end_offset = u32::try_from(assembler.offset());
    assert!(end_offset.is_ok());
    assert!(emit_aarch64(&mut assembler, &ncl_asm_aarch64::Inst::Nop));
    assert!(emit_aarch64(
        &mut assembler,
        &ncl_asm_aarch64::Inst::Ret {
            rn: ncl_asm_aarch64::Reg(30),
        }
    ));
    let blob = assembler.finish();
    assert!(blob.is_ok());
    let Ok(blob) = blob else {
        return;
    };
    let (Ok(start_offset), Ok(end_offset)) = (start_offset, end_offset) else {
        return;
    };
    let map = SafepointMap::new(end_offset, 4, 4, &[], &[], FLAG_CALL);
    assert!(map.is_ok());
    let Ok(map) = map else {
        return;
    };
    let sites = [SafepointSite::call(start_offset, end_offset)];
    assert_eq!(
        verify_safepoint_sites(
            AllocationTarget::AArch64,
            &blob.bytes,
            std::slice::from_ref(&map),
            &sites
        ),
        Ok(())
    );

    let mut corrupted_map = map;
    corrupted_map.pc_offset = end_offset.saturating_add(4);
    assert!(corrupted_map.pc_offset > end_offset);
    assert!(usize::try_from(corrupted_map.pc_offset).is_ok_and(|pc| pc < blob.bytes.len()));
    assert_eq!(
        verify_safepoint_sites(
            AllocationTarget::AArch64,
            &blob.bytes,
            &[corrupted_map],
            &sites,
        ),
        Err(CodegenError::MachineVerification(
            MachineVerificationError::MachineSafepointPcMismatch,
        ))
    );
}
