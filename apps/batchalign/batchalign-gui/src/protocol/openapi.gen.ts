// THIS FILE IS GENERATED — DO NOT EDIT BY HAND.
// Source: batchalign.api app.openapi().
// Regenerate with: just batchalign::gui openapi

export interface paths {
    "/backends": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** List Backends */
        get: operations["list_backends_backends_get"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/capabilities": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Capabilities
         * @description One-stop discovery: every recipe, every backend, input shape, job lifecycle.
         *
         *     Clients should hit this to learn what the server can do without
         *     reading the OpenAPI schema. The contents are derived entirely from
         *     introspection — adding a recipe or backend in the source updates
         *     this response automatically.
         */
        get: operations["capabilities_capabilities_get"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Health */
        get: operations["health_health_get"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/jobs/{job_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Get Job */
        get: operations["get_job_jobs__job_id__get"];
        put?: never;
        post?: never;
        /** Cancel Job */
        delete: operations["cancel_job_jobs__job_id__delete"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/jobs/{job_id}/events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Stream Events */
        get: operations["stream_events_jobs__job_id__events_get"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/jobs/{job_id}/result": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Get Result */
        get: operations["get_result_jobs__job_id__result_get"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** List Recipes */
        get: operations["list_recipes_recipes_get"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/align": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Align
         * @description Forced alignment only (`Task.Fa`).
         */
        post: operations["run_align_recipes_align_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/avqi": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Avqi
         * @description Acoustic Voice Quality Index extraction.
         */
        post: operations["run_avqi_recipes_avqi_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/compare": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Compare
         * @description Gold/main transcript comparison.
         *
         *     Wires ``[Morphosyntax, Compare]``. Morphosyntax runs on both sides of
         *     the ``Paired`` (the runner short-circuits per-utterance if `%mor:` is
         *     already present), so the compare backend can lift POS off the `%mor`
         *     tier to populate ``%xsmor``.
         *
         *     ``compare_backend`` defaults to the native Rust
         *     ``batchalign._core.CompareBackend``. ``stanza_backend`` is required
         *     for `%xsmor` to carry real POS tags.
         */
        post: operations["run_compare_recipes_compare_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/coref": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Coref
         * @description Coreference resolution.
         */
        post: operations["run_coref_recipes_coref_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/morphotag": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Morphotag
         * @description Morphosyntax tagging via Stanza (UD `%mor` / `%gra`).
         */
        post: operations["run_morphotag_recipes_morphotag_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/opensmile": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Opensmile
         * @description OpenSMILE acoustic-feature extraction. `feature_set` is a backend
         *     constructor arg (`OpenSmileBackend(feature_set="eGeMAPSv02")`).
         */
        post: operations["run_opensmile_recipes_opensmile_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/transcribe": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Transcribe
         * @description ASR + utterance segmentation (+ optional speaker diarization).
         *
         *     This is BA2's transcribe *pairing*: ASR, then a utterance-segmentation
         *     stage. Pass `utseg_backend=CHATUtteranceBackend(...)` for BA2's BERT
         *     segmenter (the parity path, applied uniformly to whichever ASR engine
         *     produced the words). Pyannote, if given as `speaker_backend`, services
         *     both Speaker and UtSeg, so it covers segmentation on its own.
         *
         *     Force-alignment is *not* wired here — compose `align(fa_backend=...)`
         *     afterwards for refined word-level timings.
         */
        post: operations["run_transcribe_recipes_transcribe_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/translate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Translate
         * @description Translate utterances. Target language is set on the backend
         *     (`GoogleTranslateBackend(target="eng")`).
         */
        post: operations["run_translate_recipes_translate_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/recipes/utseg": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Run Utseg
         * @description Utterance segmentation. `stanza_fallback` lives on the backend.
         */
        post: operations["run_utseg_recipes_utseg_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/uploads": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /** Upload */
        post: operations["upload_uploads_post"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/uploads/{upload_id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /** Delete Upload */
        delete: operations["delete_upload_uploads__upload_id__delete"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        /** AlignRequest */
        AlignRequest: {
            fa_backend: components["schemas"]["BackendSpec"];
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
        };
        /** AvqiRequest */
        AvqiRequest: {
            avqi_backend: components["schemas"]["BackendSpec"];
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
        };
        /**
         * BackendSpec
         * @description Discriminated JSON for a backend.
         *
         *     The set of valid ``kind`` strings is the set of names in
         *     :data:`BACKEND_CLASSES`. ``kwargs`` is forwarded straight to the
         *     constructor; we validate it against ``inspect.signature`` at
         *     materialization (not at request-parse) so nested backend kwargs
         *     (e.g. ``stanza_fallback``) can themselves be ``BackendSpec``.
         */
        BackendSpec: {
            /** Kind */
            kind: string;
            /** Kwargs */
            kwargs?: {
                [key: string]: unknown;
            };
        };
        /** Body_upload_uploads_post */
        Body_upload_uploads_post: {
            /** File */
            file: string;
        };
        /** CompareRequest */
        CompareRequest: {
            compare_backend?: components["schemas"]["BackendSpec"] | null;
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
            stanza_backend: components["schemas"]["BackendSpec"];
        };
        /** CorefRequest */
        CorefRequest: {
            coref_backend: components["schemas"]["BackendSpec"];
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
        };
        /** HTTPValidationError */
        HTTPValidationError: {
            /** Detail */
            detail?: components["schemas"]["ValidationError"][];
        };
        /**
         * InputSpec
         * @description Uniform input descriptor.
         *
         *     Exactly one of ``upload_id`` / ``url`` / ``path`` must be set for the
         *     primary slot. ``paired`` additionally needs a gold counterpart via
         *     ``gold_upload_id`` / ``gold_url`` / ``gold_path``.
         *
         *     ``path`` is honored only when the server is configured to trust local
         *     paths (off by default — set ``BATCHALIGN_API_ALLOW_PATHS=1``).
         */
        InputSpec: {
            /** Gold Path */
            gold_path?: string | null;
            /** Gold Upload Id */
            gold_upload_id?: string | null;
            /** Gold Url */
            gold_url?: string | null;
            /**
             * Kind
             * @default media
             * @enum {string}
             */
            kind: "media" | "chat" | "paired";
            /** Path */
            path?: string | null;
            /** Source Id */
            source_id?: string | null;
            /** Upload Id */
            upload_id?: string | null;
            /** Url */
            url?: string | null;
        };
        /** MorphotagRequest */
        MorphotagRequest: {
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
            stanza_backend: components["schemas"]["BackendSpec"];
        };
        /** OpensmileRequest */
        OpensmileRequest: {
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            opensmile_backend: components["schemas"]["BackendSpec"];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
        };
        /** TranscribeRequest */
        TranscribeRequest: {
            asr_backend: components["schemas"]["BackendSpec"];
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
            speaker_backend?: components["schemas"]["BackendSpec"] | null;
            utseg_backend?: components["schemas"]["BackendSpec"] | null;
        };
        /** TranslateRequest */
        TranslateRequest: {
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
            translate_backend: components["schemas"]["BackendSpec"];
        };
        /** UtsegRequest */
        UtsegRequest: {
            /**
             * Inputs
             * @description One or more inputs to process.
             */
            inputs: components["schemas"]["InputSpec"][];
            /**
             * Pipeline Opts
             * @description Forwarded to Pipeline(...).
             */
            pipeline_opts?: {
                [key: string]: unknown;
            } | null;
            utseg_backend: components["schemas"]["BackendSpec"];
        };
        /** ValidationError */
        ValidationError: {
            /** Context */
            ctx?: Record<string, never>;
            /** Input */
            input?: unknown;
            /** Location */
            loc: (string | number)[];
            /** Message */
            msg: string;
            /** Error Type */
            type: string;
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    list_backends_backends_get: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
        };
    };
    capabilities_capabilities_get: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
        };
    };
    health_health_get: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
        };
    };
    get_job_jobs__job_id__get: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                job_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    cancel_job_jobs__job_id__delete: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                job_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: boolean;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    stream_events_jobs__job_id__events_get: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                job_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": unknown;
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    get_result_jobs__job_id__result_get: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                job_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": unknown;
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    list_recipes_recipes_get: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
        };
    };
    run_align_recipes_align_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AlignRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    run_avqi_recipes_avqi_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AvqiRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    run_compare_recipes_compare_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CompareRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    run_coref_recipes_coref_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CorefRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    run_morphotag_recipes_morphotag_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MorphotagRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    run_opensmile_recipes_opensmile_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["OpensmileRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    run_transcribe_recipes_transcribe_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TranscribeRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    run_translate_recipes_translate_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TranslateRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    run_utseg_recipes_utseg_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UtsegRequest"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    upload_uploads_post: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "multipart/form-data": components["schemas"]["Body_upload_uploads_post"];
            };
        };
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: string;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
    delete_upload_uploads__upload_id__delete: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                upload_id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Successful Response */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: boolean;
                    };
                };
            };
            /** @description Validation Error */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HTTPValidationError"];
                };
            };
        };
    };
}
