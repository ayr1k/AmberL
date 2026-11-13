// SPDX-License-Identifier: MPL-2.0
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <shellapi.h>
#undef GetCurrentTime
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Data.Json.h>
#include <winrt/Windows.Graphics.h>
#include <winrt/Microsoft.UI.Windowing.h>
#include <winrt/Microsoft.UI.Xaml.h>
#include <winrt/Microsoft.UI.Xaml.Controls.h>
#include <winrt/Microsoft.UI.Xaml.Controls.Primitives.h>
#include <winrt/Microsoft.UI.Xaml.Markup.h>
#include <winrt/Microsoft.UI.Xaml.XamlTypeInfo.h>
#include <winrt/Windows.UI.Xaml.Interop.h>
#include <winrt/Microsoft.UI.Xaml.Automation.h>
#include <winrt/Microsoft.UI.Xaml.Media.h>
#include <chrono>
#include <filesystem>
#include <string>
#include <vector>
using namespace winrt;
using namespace Microsoft::UI::Xaml;
using namespace Microsoft::UI::Xaml::Controls;
using namespace Windows::Data::Json;
#include "layout.h"
#include "bridge.h"

struct App : ApplicationT<App, Markup::IXamlMetadataProvider> {
    XamlTypeInfo::XamlControlsXamlMetaDataProvider metadata;
    Markup::IXamlType GetXamlType(hstring const& name) {return metadata.GetXamlType(name);}
    Markup::IXamlType GetXamlType(Windows::UI::Xaml::Interop::TypeName const& name) {return metadata.GetXamlType(name);}
    com_array<Markup::XmlnsDefinition> GetXmlnsDefinitions() {return metadata.GetXmlnsDefinitions();}
    Window window{nullptr}; Grid root{nullptr}; DispatcherTimer timer{nullptr};
    TextBox path{nullptr},filter{nullptr},detail{nullptr},log{nullptr}; TextBlock status{nullptr};
    Button scan{nullptr},cancel{nullptr},startTask{nullptr},cancelTask{nullptr}; ProgressBar progress{nullptr}; ListView list{nullptr};
    Bridge bridge; std::vector<hstring> ids; uint64_t revision{},rowsRevision{}; bool updating{};
    template<typename T> T control(wchar_t const* name) {return root.FindName(name).as<T>();}
    void command(hstring const& name,hstring const& value=L"") {
        try {auto result=bridge.command(name,value); if(!result.GetNamedBoolean(L"ok")) status.Text(result.GetNamedString(L"error")); else update(result.GetNamedObject(L"snapshot"));}
        catch(hresult_error const& e) {status.Text(L"操作失败 / "+e.message());}
    }
    void update(JsonObject snapshot) {
        auto next=static_cast<uint64_t>(snapshot.GetNamedNumber(L"revision")); if(next==revision) return;
        auto rowsNext=static_cast<uint64_t>(snapshot.GetNamedNumber(L"rows_revision"));
        if(rowsNext!=rowsRevision) {
            auto full=bridge.command(L"snapshot",L"",true); if(!full.GetNamedBoolean(L"ok")) return;
            snapshot=full.GetNamedObject(L"snapshot"); next=static_cast<uint64_t>(snapshot.GetNamedNumber(L"revision"));
            auto rows=snapshot.GetNamedArray(L"rows"); std::vector<Windows::Foundation::IInspectable> items; items.reserve(rows.Size()); ids.clear();
            for(auto value:rows) {auto row=value.GetObject(); ids.push_back(row.GetNamedString(L"id")); items.push_back(box_value(row.GetNamedString(L"text")));}
            int selected=-1;
            auto selectedValue=snapshot.GetNamedValue(L"selected_id");
            if(selectedValue.ValueType()==JsonValueType::String) for(size_t i=0;i<ids.size();++i) if(ids[i]==selectedValue.GetString()) selected=static_cast<int>(i);
            updating=true; list.ItemsSource(single_threaded_observable_vector<Windows::Foundation::IInspectable>(std::move(items))); list.SelectedIndex(selected); updating=false; rowsRevision=rowsNext;
        }
        auto set=[](TextBox const& box,hstring const& text) {if(box.Text()!=text) box.Text(text);};
        set(detail,snapshot.GetNamedString(L"detail")); set(log,snapshot.GetNamedString(L"log")); status.Text(snapshot.GetNamedString(L"status"));
        bool scanning=snapshot.GetNamedBoolean(L"scanning"),running=snapshot.GetNamedBoolean(L"task_running");
        scan.IsEnabled(!scanning); cancel.IsEnabled(scanning); startTask.IsEnabled(!running); cancelTask.IsEnabled(running); progress.Value(snapshot.GetNamedNumber(L"task_progress")*100); revision=next;
    }
    void OnLaunched(LaunchActivatedEventArgs const&) {
        try {
        Resources().MergedDictionaries().Append(XamlControlsResources());
        window=Window(); window.Title(L"AmberL · WinUI 3 / App SDK 2.6.5 comparison");
        root=Markup::XamlReader::Load(layout).as<Grid>();
        path=control<TextBox>(L"Path");filter=control<TextBox>(L"Filter");detail=control<TextBox>(L"Detail");log=control<TextBox>(L"Log");status=control<TextBlock>(L"Status");list=control<ListView>(L"List");
        scan=control<Button>(L"Scan");cancel=control<Button>(L"Cancel");startTask=control<Button>(L"StartTask");cancelTask=control<Button>(L"CancelTask");progress=control<ProgressBar>(L"Progress");
        int count{}; auto args=CommandLineToArgvW(GetCommandLineW(),&count); if(args) {if(count>1) path.Text(args[1]);LocalFree(args);}
        auto weak=get_weak();
        scan.Click([weak](auto&&,auto&&){if(auto self=weak.get())self->command(L"scan",self->path.Text());});
        cancel.Click([weak](auto&&,auto&&){if(auto self=weak.get())self->command(L"cancel_scan");});
        filter.TextChanged([weak](auto&&,auto&&){if(auto self=weak.get())self->command(L"filter",self->filter.Text());});
        list.SelectionChanged([weak](auto&&,auto&&){if(auto self=weak.get()){int index=self->list.SelectedIndex();if(!self->updating&&index>=0&&static_cast<size_t>(index)<self->ids.size())self->command(L"select",self->ids[index]);}});
        startTask.Click([weak](auto&&,auto&&){if(auto self=weak.get())self->command(L"start_task");});
        cancelTask.Click([weak](auto&&,auto&&){if(auto self=weak.get())self->command(L"cancel_task");});
        control<Button>(L"About").Click([weak](auto&&,auto&&){if(auto self=weak.get()){auto text=self->control<TextBlock>(L"AboutText");text.Visibility(text.Visibility()==Visibility::Visible?Visibility::Collapsed:Visibility::Visible);}});
        window.Content(root);window.Activate();window.AppWindow().ResizeClient({1120,820});
        timer=DispatcherTimer();timer.Interval(std::chrono::milliseconds(50));timer.Tick([weak](auto&&,auto&&){if(auto self=weak.get())self->command(L"snapshot");});timer.Start();
        window.Closed([weak](auto&&,auto&&){if(auto self=weak.get())self->timer.Stop();});command(L"snapshot");
        } catch(hresult_error const& e) {
            MessageBoxW(nullptr,e.message().c_str(),L"AmberL WinUI initialization failed",MB_OK|MB_ICONERROR);
            Exit();
        }
    }
};
int WINAPI wWinMain(HINSTANCE,HINSTANCE,PWSTR,int) {
    try {init_apartment(apartment_type::single_threaded);Application::Start([](auto&&){make<App>();});return 0;}
    catch(hresult_error const& e){MessageBoxW(nullptr,e.message().c_str(),L"AmberL WinUI comparison failed",MB_OK|MB_ICONERROR);return 1;}
}
