import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import electron from 'vite-plugin-electron/simple'

export default defineConfig({
    plugins: [
        electron({
            main: {
                entry: 'src/main/index.ts',
                vite: {
                    build: {
                        rollupOptions: {
                            output: {
                                entryFileNames: 'main.js',
                            },
                            external: [
                                '@grpc/grpc-js',
                                'management-interface',
                                // 'google-protobuf',
                                'nseventforwarder',
                            ],
                        },
                    }
                }
            },
            preload: {
                input: 'src/renderer/preload.ts',
            },
        }),
        react(),
    ]
})
