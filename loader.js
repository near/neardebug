import init, { list_methods, prepare_contract, Logic, Context, Store, init_panic_hook, DebugExternal } from "./pkg/neardebug.js";

(function(window, document) {
    async function run(method_name) {
        const contract = window.contract;
        const memory = new WebAssembly.Memory({ initial: 1024, maximum: 2048 });
        contract.memory = memory;
        const context = new Context().input_str(document.querySelector("#input").value);
        const protocol_version = 72;
        const ext = new DebugExternal(contract.store, context, protocol_version);
        const logic = new Logic(context, memory, ext);
        contract.logic = logic;

        const import_object = { env: {} };
        import_object.internal = {
            finite_wasm_gas: (...args) => logic.finite_wasm_gas(...args),
            finite_wasm_stack: (...args) => logic.finite_wasm_stack(...args),
            finite_wasm_unstack: (...args) => logic.finite_wasm_unstack(...args),
        };
        import_object.env = {
            memory: memory,
            log_utf8: (len, ptr) => {
                try {
                    console.log(logic.get_utf8_string_free(len, ptr));
                } catch { }
                return logic.log_utf8(len, ptr);
            },

            log_utf16: (len, ptr) => {
                console.log("TODO: output log_utf16 to console");
                return logic.log_utf16(len, ptr)
            },

            read_register: (...args) => logic.read_register(...args),
            register_len: (...args) => logic.register_len(...args),
            write_register: (...args) => logic.write_register(...args),
            current_account_id: (...args) => logic.current_account_id(...args),
            signer_account_id: (...args) => logic.signer_account_id(...args),
            signer_account_pk: (...args) => logic.signer_account_pk(...args),
            predecessor_account_id: (...args) => logic.predecessor_account_id(...args),
            input: (...args) => logic.input(...args),
            block_index: (...args) => logic.block_index(...args),
            block_timestamp: (...args) => logic.block_timestamp(...args),
            epoch_height: (...args) => logic.epoch_height(...args),
            storage_usage: (...args) => logic.storage_usage(...args),
            account_balance: (...args) => logic.account_balance(...args),
            account_locked_balance: (...args) => logic.account_locked_balance(...args),
            attached_deposit: (...args) => logic.attached_deposit(...args),
            prepaid_gas: (...args) => logic.prepaid_gas(...args),
            used_gas: (...args) => logic.used_gas(...args),
            random_seed: (...args) => logic.random_seed(...args),
            sha256: (...args) => logic.sha256(...args),
            keccak256: (...args) => logic.keccak256(...args),
            keccak512: (...args) => logic.keccak512(...args),
            ed25519_verify: (...args) => logic.ed25519_verify(...args),
            ripemd160: (...args) => logic.ripemd160(...args),
            ecrecover: (...args) => logic.ecrecover(...args),
            promise_create: (...args) => logic.promise_create(...args),
            promise_then: (...args) => logic.promise_then(...args),
            promise_and: (...args) => logic.promise_and(...args),
            promise_batch_create: (...args) => logic.promise_batch_create(...args),
            promise_batch_then: (...args) => logic.promise_batch_then(...args),
            promise_batch_action_create_account: (...args) => logic.promise_batch_action_create_account(...args),
            promise_batch_action_deploy_contract: (...args) => logic.promise_batch_action_deploy_contract(...args),
            promise_batch_action_function_call: (...args) => logic.promise_batch_action_function_call(...args),
            promise_batch_action_function_call_weight: (...args) => logic.promise_batch_action_function_call_weight(...args),
            promise_batch_action_transfer: (...args) => logic.promise_batch_action_transfer(...args),
            promise_batch_action_stake: (...args) => logic.promise_batch_action_stake(...args),
            promise_batch_action_add_key_with_full_access: (...args) => logic.promise_batch_action_add_key_with_full_access(...args),
            promise_batch_action_add_key_with_function_call: (...args) => logic.promise_batch_action_add_key_with_function_call(...args),
            promise_batch_action_delete_key: (...args) => logic.promise_batch_action_delete_key(...args),
            promise_batch_action_delete_account: (...args) => logic.promise_batch_action_delete_account(...args),
            promise_yield_create: (...args) => logic.promise_yield_create(...args),
            promise_yield_resume: (...args) => logic.promise_yield_resume(...args),
            promise_results_count: () => logic.promise_results_count(...args),
            promise_result: (...args) => logic.promise_result(...args),
            promise_return: (...args) => logic.promise_return(...args),
            value_return: (...args) => logic.value_return(...args),
            panic: (...args) => logic.panic(...args),
            panic_utf8: (...args) => logic.panic_utf8(...args),
            abort: (...args) => logic.abort(...args),
            storage_write: (...args) => logic.storage_write(...args),
            storage_read: (...args) => logic.storage_read(...args),
            storage_remove: (...args) => logic.storage_remove(...args),
            storage_has_key: (...args) => logic.storage_has_key(...args),
            storage_iter_prefix: (...args) => logic.storage_iter_prefix(...args),
            storage_iter_range: (...args) => logic.storage_iter_range(...args),
            storage_iter_next: (...args) => logic.storage_iter_next(...args),
            gas: (...args) => logic.gas(...args),
            burn_gas: (...args) => logic.burn_gas(...args),
            validator_stake: (...args) => logic.validator_stake(...args),
            validator_total_stake: (...args) => logic.validator_total_stake(...args),
            alt_bn128_g1_multiexp: (...args) => logic.alt_bn128_g1_multiexp(...args),
            alt_bn128_g1_sum: (...args) => logic.alt_bn128_g1_sum(...args),
            alt_bn128_pairing_check: (...args) => logic.alt_bn128_pairing_check(...args),
            bls12381_p1_sum: (value_len, value_ptr, register_id) /* ->  [u64] */ => { console.log("TODO bls12381_p1_sum"); },
            bls12381_p2_sum: (value_len, value_ptr, register_id) /* ->  [u64] */ => { console.log("TODO bls12381_p2_sum"); },
            bls12381_g1_multiexp: (value_len, value_ptr, register_id) /* ->  [u64] */ => { console.log("TODO bls12381_g1_multiexp"); },
            bls12381_g2_multiexp: (value_len, value_ptr, register_id) /* ->  [u64] */ => { console.log("TODO bls12381_g2_multiexp"); },
            bls12381_map_fp_to_g1: (value_len, value_ptr, register_id) /* ->  [u64] */ => { console.log("TODO bls12381_map_fp_to_g1"); },
            bls12381_map_fp2_to_g2: (value_len, value_ptr, register_id) /* ->  [u64] */ => { console.log("TODO bls12381_map_fp2_to_g2"); },
            bls12381_pairing_check: (value_len, value_ptr) /* ->  [u64] */ => { console.log("TODO bls12381_pairing_check"); },
            bls12381_p1_decompress: (value_len, value_ptr, register_id) /* ->  [u64] */ => { console.log("TODO bls(12381_p1_decompress"); },
            bls12381_p2_decompress: (value_len, value_ptr, register_id) /* ->  [u64] */ => { console.log("TODO bls12381_p2_decompress"); },
            sandbox_debug_log: (len, ptr) /* ->  [] */ => { console.log("TODO sandbox_debug_log"); },
            sleep_nanos: (duration) /* ->  [] */ => { console.log("TODO sleep_nanos"); },
        };
        window.contract.instance = await WebAssembly.instantiate(window.contract.module, import_object);
        window.contract.instance.exports[method_name]();
    }

    async function load(contract_data) {
        delete contract.instance;
        delete contract.memory;
        delete contract.logic;
        if (contract_data === undefined) {
            delete contract.module;
            return;
        }
        contract_data = new Uint8Array(contract_data);
        const prepared_contract_data = prepare_contract(contract_data);
        contract.module = await WebAssembly.compile(prepared_contract_data);
    }

    async function on_contract_change(element) {
        const button = document.querySelector("#execute");
        const method_selector = document.querySelector("#methods");
        button.disabled = true;
        method_selector.innerHTML = "";
        if (element.files.length > 0) {
            const buffer = await element.files[0].arrayBuffer();
            const callable_methods = list_methods(new Uint8Array(buffer));
            for (const method of callable_methods) {
                const option = document.createElement("option");
                option.innerText = method;
                option.value = method;
                method_selector.appendChild(option);
            }
            await load(buffer);
        } else {
            await load(undefined);
        }
        button.disabled = false;
    }

    async function on_load() {
        await init();
        init_panic_hook();
        window.contract = {
            store: new Store(),
        };
        const form = document.querySelector('#contract_form');
        form.addEventListener('submit', async (e) => {
            e.preventDefault();
            const methods = document.querySelector("#methods");
            const method = methods.selectedOptions[0].value;
            run(method);
        });

        const file_input = document.querySelector('#contract');
        file_input.addEventListener('change', (e) => {
            on_contract_change(e.target);
        });
        on_contract_change(file_input);
    }

    (window.addEventListener || window.attachEvent)('load', on_load);
})(window, document);
