// SPDX-License-Identifier: MPL-2.0
class Bridge {
    HMODULE library{};
    uint64_t id{};
    using Create=uint64_t(__cdecl*)(); using Destroy=void(__cdecl*)(uint64_t);
    using Call=intptr_t(__cdecl*)(uint64_t,const uint8_t*,size_t);
    using Read=intptr_t(__cdecl*)(uint64_t,uint8_t*,size_t);
    Destroy destroy{}; Call call{}; Read read{};
    std::vector<uint8_t> response;
public:
    Bridge() {
        wchar_t executable[32768]{};
        auto n=GetModuleFileNameW(nullptr,executable,32768); if(!n || n==32768) throw hresult_error(E_FAIL,L"Executable path unavailable");
        auto path=std::filesystem::path(executable).parent_path()/L"amberl_ui_bridge.dll";
        library=LoadLibraryExW(path.c_str(),nullptr,LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR|LOAD_LIBRARY_SEARCH_SYSTEM32);
        if(!library) throw hresult_error(HRESULT_FROM_WIN32(GetLastError()),L"Ambrum comparison bridge unavailable");
        auto create=reinterpret_cast<Create>(GetProcAddress(library,"amberl_ui_create"));
        destroy=reinterpret_cast<Destroy>(GetProcAddress(library,"amberl_ui_destroy"));
        call=reinterpret_cast<Call>(GetProcAddress(library,"amberl_ui_call")); read=reinterpret_cast<Read>(GetProcAddress(library,"amberl_ui_read"));
        if(!create||!destroy||!call||!read) {FreeLibrary(library);library=nullptr;throw hresult_error(E_FAIL,L"Incompatible Ambrum bridge");}
        id=create(); if(!id) {FreeLibrary(library);library=nullptr;throw hresult_error(E_FAIL,L"Cannot create comparison model");}
    }
    ~Bridge() {if(id&&destroy) destroy(id); if(library) FreeLibrary(library);}
    Bridge(Bridge const&)=delete; Bridge& operator=(Bridge const&)=delete;
    JsonObject command(hstring const& name,hstring const& value=L"",bool rows=false) {
        JsonObject request; request.Insert(L"command",JsonValue::CreateStringValue(name)); request.Insert(L"value",JsonValue::CreateStringValue(value)); request.Insert(L"rows",JsonValue::CreateBooleanValue(rows));
        auto input=to_string(request.Stringify());
        auto size=call(id,reinterpret_cast<const uint8_t*>(input.data()),input.size());
        if(size<=0||size>16*1024*1024) throw hresult_error(E_FAIL,L"Comparison command failed");
        response.resize(static_cast<size_t>(size));
        if(read(id,response.data(),response.size())!=size) throw hresult_error(E_FAIL,L"Comparison response unavailable");
        return JsonObject::Parse(to_hstring(std::string_view(reinterpret_cast<const char*>(response.data()),response.size())));
    }
};
