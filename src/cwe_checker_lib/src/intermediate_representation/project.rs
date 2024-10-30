use super::*;

use crate::utils::debug;
use crate::utils::log::WithLogs;

use std::collections::{BTreeMap, BTreeSet};

mod ir_passes;
use ir_passes::*;

/// The `Project` struct is the main data structure representing a binary.
///
/// It contains information about the disassembled binary
/// and about the execution environment of the binary.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Project {
    /// All (known) executable code of the binary is contained in the `program` term.
    pub program: Term<Program>,
    /// The CPU architecture on which the binary is assumed to be executed.
    pub cpu_architecture: String,
    /// The stack pointer register for the given CPU architecture.
    pub stack_pointer_register: Variable,
    /// The known calling conventions that may be used for calls to extern functions.
    pub calling_conventions: BTreeMap<String, CallingConvention>,
    /// The set of all known physical registers for the CPU architecture.
    /// Does only contain base registers, i.e. sub registers of other registers are not contained.
    pub register_set: BTreeSet<Variable>,
    /// Contains the properties of C data types. (e.g. size)
    pub datatype_properties: DatatypeProperties,
    /// Represents the memory after loading the binary.
    pub runtime_memory_image: RuntimeMemoryImage,
}

impl Project {
    /// Return the size (in bytes) for pointers of the given architecture.
    pub fn get_pointer_bytesize(&self) -> ByteSize {
        self.stack_pointer_register.size
    }

    /// Try to guess a standard calling convention from the list of calling conventions in the project.
    pub fn get_standard_calling_convention(&self) -> Option<&CallingConvention> {
        self.calling_conventions
            .get("__stdcall")
            .or_else(|| self.calling_conventions.get("__cdecl"))
            .or_else(|| self.calling_conventions.get("__thiscall")) // for x86_64 Microsoft Windows binaries.
    }

    /// Try to find a specific calling convention in the list of calling conventions in the project.
    /// If not given a calling convention (i.e. given `None`) or the given calling convention name was not found
    /// then falls back to `get_standard_calling_convention`.
    pub fn get_specific_calling_convention(
        &self,
        cconv_name_opt: &Option<String>,
    ) -> Option<&CallingConvention> {
        // FIXME: On x86 Windows binaries we can get a strange edge case:
        // For some reason we get cases where Ghidra annotates a function with `__cdecl` as calling convention,
        // but the general calling convention list only contains `__fastcall` and `__thiscall`.
        // We should investigate this, so that we do not have to fall back to the standard calling convention.
        cconv_name_opt
            .as_ref()
            .and_then(|cconv_name| self.calling_conventions.get(cconv_name))
            .or_else(|| self.get_standard_calling_convention())
    }

    /// Return the calling convention associated to the given extern symbol.
    /// If the extern symbol has no annotated calling convention
    /// then return the standard calling convention of the project instead.
    ///
    /// This function panics if no suitable calling convention is found.
    pub fn get_calling_convention(&self, extern_symbol: &ExternSymbol) -> &CallingConvention {
        if let Some(cconv_name) = &extern_symbol.calling_convention {
            self.calling_conventions.get(cconv_name).unwrap()
        } else {
            self.get_standard_calling_convention().unwrap()
        }
    }
}

impl WithLogs<Project> {
    /// Performs only the optimizing normalization passes.
    ///
    /// [`Project::normalize_basic`] **must** be called before this method.
    ///
    /// Runs only the optimization passes that transform the program to an
    /// equivalent, simpler representation. This step is exprected to improve
    /// the speed and precision of later analyses.
    ///
    /// Currently, the following optimizations are performed:
    ///
    /// - Propagate input expressions along variable assignments.
    /// - Replace trivial expressions like `a XOR a` with their result.
    /// - Remove dead register assignments.
    /// - Propagate the control flow along chains of conditionals with the same condition.
    /// - Substitute bitwise `AND` and `OR` operations with the stack pointer
    ///   in cases where the result is known due to known stack pointer alignment.
    pub fn optimize(&mut self, debug_settings: &debug::Settings) {
        let mut logs = Vec::new();

        run_ir_pass![
            self.program.term,
            (),
            IntraproceduralDeadBlockElimPass,
            logs,
            debug_settings,
        ];
        run_ir_pass![
            self.program.term,
            (),
            InputExpressionPropagationPass,
            logs,
            debug_settings,
        ];
        run_ir_pass![
            self.program.term,
            (),
            TrivialExpressionSubstitutionPass,
            logs,
            debug_settings,
        ];
        run_ir_pass![
            self.program.term,
            self.register_set,
            DeadVariableElimPass,
            logs,
            debug_settings,
        ];
        run_ir_pass![
            self.program.term,
            (),
            ControlFlowPropagationPass,
            logs,
            debug_settings,
        ];
        run_ir_pass![
            self.program.term,
            self,
            StackPointerAlignmentSubstitutionPass,
            logs,
            debug_settings,
        ];

        debug_assert_postconditions![self.program.term, (), IntraproceduralDeadBlockElimPass];
        debug_assert_postconditions![self.program.term, (), InputExpressionPropagationPass];
        debug_assert_postconditions![self.program.term, (), TrivialExpressionSubstitutionPass];
        debug_assert_postconditions![self.program.term, self.register_set, DeadVariableElimPass];
        debug_assert_postconditions![self.program.term, (), ControlFlowPropagationPass];
        debug_assert_postconditions![
            self.program.term,
            self,
            StackPointerAlignmentSubstitutionPass,
        ];

        self.add_logs(logs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retarget_nonexisting_jumps() {
        let mut jmp_term = Term {
            tid: Tid::new("jmp"),
            term: Jmp::Branch(Tid::new("nonexisting_target")),
        };
        assert_eq!(jmp_term.term, Jmp::Branch(Tid::new("nonexisting_target")));
        assert!(jmp_term
            .retarget_nonexisting_jump_targets_to_artificial_sink(&HashSet::new(),)
            .is_err());
        assert_eq!(jmp_term.term, Jmp::Branch(Tid::artificial_sink_block("")));
    }
}
